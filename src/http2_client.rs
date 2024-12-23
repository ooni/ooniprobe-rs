use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::client::conn::http2;
use hyper_util::rt::TokioIo;

use boring::ssl::{
    SslConnector, SslCurve, SslMethod, SslOptions, SslSignatureAlgorithm, SslVersion,
};
use std::net::ToSocketAddrs;
use tokio::net::TcpStream;

#[derive(Debug, Clone)]
pub enum Http2Error {
    GenericError,
    CreatingRequest,
    SendingRequest,
    ReadingBody,
}

pub struct Http2Client {
    sender: http2::SendRequest<Full<Bytes>>,
    host: Arc<str>,
    port: u32,
}

impl Http2Client {
    pub async fn connect(host: Arc<str>, port: u32) -> Result<Http2Client, Http2Error> {
        let addr = format!("{}:{}", host, port)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap();
        let stream = TcpStream::connect(&addr).await.unwrap();

        // Setup TLS stack to parrot latest chrome.
        // ja4 hash matches chrome 131.0.6778.70.
        // TODO:
        // * re-enable compression
        // * probably expose this particular configuration inside of
        //   SslConnector::builder() or ConnectConfiguration
        let mut connector = SslConnector::builder(SslMethod::tls()).unwrap();
        connector.clear_options(
            SslOptions::NO_SSLV2
                | SslOptions::NO_SSLV3
                | SslOptions::NO_TLSV1
                | SslOptions::NO_TLSV1_1,
        );
        connector
            .set_cipher_list("ALL:!aPSK:!ECDSA+SHA1:!3DES")
            .expect("failure to set_cipher_list");
        connector.set_grease_enabled(true);
        connector
            .set_min_proto_version(Some(SslVersion::TLS1_2))
            .expect("failure to set min proto version");
        connector
            .set_max_proto_version(Some(SslVersion::TLS1_3))
            .expect("failure to set max proto version");
        connector.enable_signed_cert_timestamps();
        connector
            .set_alpn_protos(b"\x02h2\x08http/1.1")
            .expect("failure to set_alpn_protos");
        connector.enable_ocsp_stapling();

        // TODO: temporarily disable while I figure out how to integrate brotli into upstream
        //connector.add_certificate_compression_algorithm(BrotliCompressor::default());

        connector
            .set_verify_algorithm_prefs(&[
                SslSignatureAlgorithm::ECDSA_SECP256R1_SHA256,
                SslSignatureAlgorithm::RSA_PSS_RSAE_SHA256,
                SslSignatureAlgorithm::RSA_PKCS1_SHA256,
                SslSignatureAlgorithm::ECDSA_SECP384R1_SHA384,
                SslSignatureAlgorithm::RSA_PSS_RSAE_SHA384,
                SslSignatureAlgorithm::RSA_PKCS1_SHA384,
                SslSignatureAlgorithm::RSA_PSS_RSAE_SHA512,
                SslSignatureAlgorithm::RSA_PKCS1_SHA512,
            ])
            .expect("failure to set verify algorithms");

        connector
            .set_curves(&[
                SslCurve::X25519_MLKEM768,
                SslCurve::X25519,
                SslCurve::SECP256R1,
                SslCurve::SECP384R1,
            ])
            .expect("failure to set curves");

        let mut config = connector.build().configure().unwrap();
        config
            .add_application_settings(b"h2", b"")
            .expect("failure to add_application_settings");
        config
            .enable_ech_grease()
            .expect("failure to enable ech_grease");

        let stream = tokio_boring::connect(config, host.as_ref(), stream)
            .await
            .unwrap();

        let io: TokioIo<tokio_boring::SslStream<TcpStream>> = TokioIo::new(stream);
        let executor = hyper_util::rt::tokio::TokioExecutor::new();

        let (sender, conn) =
            hyper::client::conn::http2::handshake::<_, _, Full<Bytes>>(executor, io)
                .await
                .map_err(|_| Http2Error::GenericError)?;

        tokio::task::spawn(async move {
            if let Err(e) = conn.await {
                println!("Error: {:?}", e);
            }
        });

        Ok(Http2Client { sender, host, port })
    }

    pub async fn send_request_read_response(
        &self,
        path_and_query: http::uri::PathAndQuery,
        method: http::Method,
        headers: http::HeaderMap,
        body: Bytes,
    ) -> Result<Bytes, Http2Error> {
        let uri = format!("https://{}:{}{}", self.host, self.port, path_and_query);

        let mut req_builder = http::Request::builder()
            .method(method)
            .uri(uri)
            .version(hyper::Version::HTTP_2)
            .header(http::header::CONTENT_LENGTH, body.len());

        req_builder
            .headers_mut()
            .ok_or(Http2Error::CreatingRequest)?
            .extend(headers);

        let req = req_builder
            .body(Full::new(body))
            .map_err(|_| Http2Error::CreatingRequest)?;

        let res = self
            .sender
            .clone()
            .send_request(req)
            .await
            .map_err(|_| Http2Error::SendingRequest)?;

        println!("Response: {:#?}", res);

        let body = res
            .collect()
            .await
            .map_err(|_| Http2Error::ReadingBody)?
            .to_bytes();

        println!("{}", String::from_utf8_lossy(&body));
        Ok(body)
    }
}

pub async fn connect_and_send_request(
    host: Arc<str>,
    port: u32,
    path_and_query: http::uri::PathAndQuery,
    method: http::Method,
    headers: http::HeaderMap,
    body: Bytes,
) -> Result<Bytes, Http2Error> {
    let client = Http2Client::connect(host, port).await?;
    let resp = client
        .send_request_read_response(path_and_query, method, headers, body)
        .await?;
    Ok(resp)
}

#[cfg(test)]
mod tests {

    use http::{uri::PathAndQuery, HeaderMap, Method};

    use super::*;

    #[tokio::test]
    async fn test_http2_client() {
        let http2_client = Http2Client::connect("tls.peet.ws".into(), 443)
            .await
            .unwrap();

        let body = http2_client
            .send_request_read_response(
                PathAndQuery::from_static("/api/all"),
                Method::GET,
                HeaderMap::new(),
                Bytes::new(),
            )
            .await
            .unwrap();
    }
}
