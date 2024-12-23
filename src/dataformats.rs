use boring::{base64, derive};

use crate::tracing::NetworkEvent;

#[derive(Default, Debug)]
pub struct DnsQuery {
    pub answers: Vec<DnsAnswer>,
    pub engine: String,
    pub failure: String,
    pub getaddrinfo_error: i32,
    pub hostname: String,
    pub query_type: String,
    pub raw_response: String,
    pub rcode: i32,
    pub resolver_address: String,
    pub t0: f64,
    pub t: f64,
    pub tags: Vec<String>,
    pub transaction_id: i32,
}

#[derive(Default, Debug)]
pub struct DnsAnswer {
    pub answer_type: String,
    pub asn: i32,
    pub as_org_name: String,
    pub hostname: String,
    pub ipv4: String,
    pub ipv6: String,
    pub ttl: Option<i32>,
}

#[derive(Default, Debug)]
pub struct HttpRequest {
    pub body: String,
    pub body_is_truncated: bool,
    pub headers_list: Vec<(String, String)>,
    pub headers: std::collections::HashMap<String, String>,
    pub method: String,
    pub tor: TorInfo,
    pub x_transport: String,
    pub url: String,
}

#[derive(Default, Debug)]
pub struct HttpResponse {
    pub body: String,
    pub body_is_truncated: bool,
    pub code: i32,
    pub headers_list: Vec<(String, String)>,
    pub headers: std::collections::HashMap<String, String>,
}

#[derive(Default, Debug)]
pub struct TorInfo {
    pub exit_ip: Option<String>,
    pub exit_name: Option<String>,
    pub is_tor: bool,
}

#[derive(Default, Debug)]
pub struct HttpTransaction {
    pub network: String,
    pub address: String,
    pub alpn: String,
    pub failure: Option<String>,
    pub request: HttpRequest,
    pub response: HttpResponse,
    pub t0: f64,
    pub t: f64,
    pub transaction_id: i32,
}

#[derive(Default, Debug)]
pub struct Measurement {
    pub annotations: std::collections::HashMap<String, String>,
    pub extensions: std::collections::HashMap<String, String>,
    pub input: String,
    pub measurement_start_time: String,
    pub probe_asn: String,
    pub probe_network_name: String,
    pub probe_cc: String,
    pub probe_city: String,
    pub probe_ip: String,
    pub report_filename: String,
    pub report_id: String,
    pub resolver_asn: String,
    pub resolver_ip: String,
    pub resolver_network_name: String,
    pub software_name: String,
    pub software_version: String,
    pub test_helpers: std::collections::HashMap<String, String>,
    pub test_keys: std::collections::HashMap<String, String>,
    pub test_name: String,
    pub test_runtime: f64,
    pub test_start_time: String,
    pub test_version: String,
}

#[derive(Default, Debug)]
pub struct TCPConnectStatus {
    pub failure: String,
    pub success: bool,
}

#[derive(Default, Debug)]
pub struct TCPConnect {
    pub ip: String,
    pub port: i32,
    pub status: TCPConnectStatus,
    pub t0: f64,
    pub t: f64,
    pub tags: Vec<String>,
    pub transaction_id: i32,
}

#[derive(Default, Debug)]
pub struct TlsHandshake {
    pub network: String,
    pub address: String,
    pub failure: String,
    pub t0: f64,
    pub t: f64,
    pub tags: Vec<String>,
    pub transaction_id: u32,

    pub no_tls_verify: bool,

    pub negotiated_protocol: String,
    pub echconfig: String,
    pub peer_certificates: Vec<String>,
    pub server_name: String,
    pub tls_version: String,
    pub cipher_suite: String,
}

impl TlsHandshake {
    pub fn new(address: &str) -> TlsHandshake {
        let mut tls = TlsHandshake::default();
        tls.address = address.to_string();
        tls
    }

    pub fn add_ssl(&mut self, ssl: &boring::ssl::SslRef) {
        self.cipher_suite = ssl
            .current_cipher()
            .map_or(String::new(), |c| c.name().to_string());
        self.negotiated_protocol = ssl
            .selected_alpn_protocol()
            .map_or(String::new(), |p| String::from_utf8_lossy(p).to_string());
        self.peer_certificates = ssl.peer_cert_chain().map_or(Vec::new(), |chain| {
            chain
                .iter()
                .map(|cert| base64::encode_block(cert.to_der().unwrap_or_default().as_ref()))
                .collect()
        });
        self.server_name = ssl
            .servername(boring::ssl::NameType::HOST_NAME)
            .map_or(String::new(), |name| name.to_string());
        self.tls_version = ssl.version_str().to_string();
    }

    pub fn add_network_event(&mut self, network_event: &NetworkEvent) {
        self.t0 = network_event.t0.unwrap();
        self.t = network_event.t.unwrap();
        self.network = network_event.proto.clone().unwrap();
        self.transaction_id = network_event.transaction_id;
    }
}
