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

pub struct DnsAnswer {
    pub answer_type: String,
    pub asn: i32,
    pub as_org_name: String,
    pub hostname: String,
    pub ipv4: String,
    pub ipv6: String,
    pub ttl: Option<i32>,
}

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

pub struct HttpResponse {
    pub body: String,
    pub body_is_truncated: bool,
    pub code: i32,
    pub headers_list: Vec<(String, String)>,
    pub headers: std::collections::HashMap<String, String>,
}

pub struct TorInfo {
    pub exit_ip: Option<String>,
    pub exit_name: Option<String>,
    pub is_tor: bool,
}

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

pub struct TCPConnectStatus {
    pub failure: String,
    pub success: bool,
}

pub struct TCPConnect {
    pub ip: String,
    pub port: i32,
    pub status: TCPConnectStatus,
    pub t0: f64,
    pub t: f64,
    pub tags: Vec<String>,
    pub transaction_id: i32,
}

pub struct TlsHandshake {
    pub network: String,
    pub address: String,
    pub cipher_suite: String,
    pub conn_id: i32,
    pub failure: String,
    pub so_error: String,
    pub negotiated_protocol: String,
    pub no_tls_verify: bool,
    pub peer_certificates: Vec<String>,
    pub server_name: String,
    pub echconfig: String,
    pub t0: f64,
    pub t: f64,
    pub tags: Vec<String>,
    pub tls_version: String,
    pub transaction_id: i32,
}
