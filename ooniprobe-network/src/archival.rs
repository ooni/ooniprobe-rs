//! OONI archival data-format types.
//!
//! <https://github.com/ooni/spec/tree/master/data-formats>.
//!
//! All types implement `Serialize`/`Deserialize` with field names that
//! match the JSON keys mandated by the spec.

use std::time::SystemTime;

use crate::utils::{b64_decode, b64_encode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A byte string that serialises as `{"format":"base64","data":"…"}` per
/// df-001-httpt § MaybeBinaryData.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BinaryData(pub Vec<u8>);

impl Serialize for BinaryData {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        if self.0.is_empty() {
            return s.serialize_none();
        }
        let mut map = s.serialize_map(Some(2))?;
        map.serialize_entry("format", "base64")?;
        map.serialize_entry("data", &b64_encode(&self.0))?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for BinaryData {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        #[derive(Deserialize)]
        struct Repr {
            format: String,
            data: String,
        }
        let repr = Option::<Repr>::deserialize(d)?;
        match repr {
            None => Ok(BinaryData(vec![])),
            Some(r) => {
                if r.format != "base64" {
                    return Err(D::Error::custom(format!(
                        "unknown binary data format: {}",
                        r.format
                    )));
                }
                let bytes = b64_decode(&r.data).map_err(D::Error::custom)?;
                Ok(BinaryData(bytes))
            }
        }
    }
}

/// A body that is either valid UTF-8 text or raw bytes (base64-encoded).
/// Mirrors the MaybeBinaryData logic in the Go implementation.
#[derive(Debug, Clone, Default)]
pub struct MaybeBinaryData(pub Vec<u8>);

impl Serialize for MaybeBinaryData {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match std::str::from_utf8(&self.0) {
            Ok(text) => s.serialize_str(text),
            Err(_) => BinaryData(self.0.clone()).serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for MaybeBinaryData {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let v = serde_json::Value::deserialize(d)?;
        match v {
            serde_json::Value::String(s) => Ok(MaybeBinaryData(s.into_bytes())),
            serde_json::Value::Object(_) => {
                let bd: BinaryData = serde_json::from_value(v).map_err(D::Error::custom)?;
                Ok(MaybeBinaryData(bd.0))
            }
            serde_json::Value::Null => Ok(MaybeBinaryData(vec![])),
            other => Err(D::Error::custom(format!(
                "unexpected body value: {}",
                other
            ))),
        }
    }
}

/// DnsAnswer: https://github.com/ooni/spec/blob/master/data-formats/df-002-dnst.md#answer
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DnsAnswer {
    pub answer_type: String,
    pub asn: i64,
    pub as_org_name: String,
    pub expiration_limit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_ttl: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responsible_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_interval: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<u32>,
    pub ttl: Option<u32>,
}

/// DnsLookupResult: https://github.com/ooni/spec/blob/master/data-formats/df-002-dnst.md#dns-data-format
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DnsLookupResult {
    pub answers: Vec<DnsAnswer>,
    pub engine: String,
    pub failure: Option<String>,
    pub hostname: String,
    pub query_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_response: Option<BinaryData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rcode: Option<i64>,
    pub resolver_hostname: Option<String>,
    pub resolver_port: Option<String>,
    pub resolver_address: String,
    pub t0: f64,
    pub t: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<i64>,
}

/// TcpConnectStatus: https://github.com/ooni/spec/blob/master/data-formats/df-005-tcpconnect.md#status
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TcpConnectStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked: Option<bool>,
    pub failure: Option<String>,
    pub success: bool,
}

/// TcpConnectResult: https://github.com/ooni/spec/blob/master/data-formats/df-005-tcpconnect.md#tcpconnect
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TcpConnectResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conn_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dial_id: Option<u16>,
    pub ip: String,
    pub port: u16,
    pub status: TcpConnectStatus,
    pub t0: f64,
    pub t: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TlsNetwork {
    Tcp,
    Udp,
}

/// TlsHandshakeResult: https://github.com/ooni/spec/blob/master/data-formats/df-006-tlshandshake.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsHandshakeResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<TlsNetwork>,
    pub address: String,
    pub cipher_suite: String,
    pub failure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub so_error: Option<String>,
    pub negotiated_protocol: String,
    pub no_tls_verify: bool,
    pub peer_certificates: Vec<BinaryData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outer_server_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ech_config: Option<String>,
    pub t0: f64,
    pub t: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    pub tls_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkOperation {
    BytesReceivedCumulative,
    Connect,
    Read,
    ReadFrom,
    Write,
    WriteTo,
    TlsHandshakeStart,
    TlsHandshakeDone,
    Close, // NOTE: addition to ooni/spec
}

/// NetworkEvent: https://github.com/ooni/spec/blob/master/data-formats/df-008-netevents.md
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conn_id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dial_id: Option<u16>,
    pub failure: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_bytes: Option<i64>,
    pub operation: NetworkOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proto: Option<String>,
    pub t0: f64,
    pub t: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<i64>,
}

/// HttpRequest: https://github.com/ooni/spec/blob/master/data-formats/df-001-httpt.md#request
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpRequest {
    pub url: String,
    pub body: MaybeBinaryData,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_is_truncated: Option<bool>,
    pub headers_list: Vec<(String, String)>,
    pub headers: std::collections::HashMap<String, String>,
    pub method: String,
    pub x_transport: String,
}

/// HttpResponse: https://github.com/ooni/spec/blob/master/data-formats/df-001-httpt.md#response
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpResponse {
    pub body: MaybeBinaryData,
    pub body_is_truncated: bool,
    pub code: u16,
    pub headers_list: Vec<(String, String)>,
    pub headers: std::collections::HashMap<String, String>,

    #[serde(skip)]
    pub raw_body: Vec<u8>,
}

/// HttpTransaction: https://github.com/ooni/spec/blob/master/data-formats/df-001-httpt.md
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HttpTransaction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpn: Option<String>,
    pub failure: Option<String>,
    pub request: HttpRequest,
    pub response: HttpResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_length: Option<u32>,
    pub t0: f64,
    pub t: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<i64>,
}

/// Top-level OONI measurement envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub annotations: std::collections::HashMap<String, String>,
    pub data_format_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<std::collections::HashMap<String, i64>>,
    pub input: Option<String>,
    #[serde(serialize_with = "serialize_time")]
    pub measurement_start_time: SystemTime,
    pub probe_asn: String,
    pub probe_cc: String,
    pub probe_ip: String,
    pub probe_network_name: String,
    pub resolver_asn: String,
    pub resolver_ip: String,
    pub resolver_network_name: String,
    pub software_name: String,
    pub software_version: String,
    pub test_helpers: serde_json::Value,
    pub test_keys: serde_json::Value,
    pub test_name: String,
    pub test_runtime: f64,
    #[serde(serialize_with = "serialize_time")]
    pub test_start_time: SystemTime,
    pub test_version: String,
}

fn serialize_time<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let dt: DateTime<Utc> = (*time).into();
    serializer.serialize_str(&dt.format("%Y-%m-%d %H:%M:%S").to_string())
}
