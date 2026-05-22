//! OONI measurement assembly.
//!
//! This module bridges the experiment executor and the OONI archival format.

use std::time::{Instant, SystemTime};

use serde::Serialize;

use crate::{
    experiment::Experiment,
    parser::{ExperimentConfig, ParseError},
};

use ooniprobe_network::{
    archival::{
        DnsLookupResult, HttpTransaction, Measurement, NetworkEvent, TcpConnectResult,
        TlsHandshakeResult,
    },
    trace::Trace,
};

/// The `test_keys` object written into every OONI measurement.
#[derive(Debug, Default, Serialize)]
pub struct TestKeys {
    pub queries: Vec<DnsLookupResult>,
    pub tcp_connect: Vec<TcpConnectResult>,
    pub tls_handshakes: Vec<TlsHandshakeResult>,
    pub network_events: Vec<NetworkEvent>,
    pub requests: Vec<HttpTransaction>,
}

impl TestKeys {
    /// Drain all observations out of `trace` into a new `TestKeys`.
    pub fn from_trace(trace: &Trace) -> Self {
        Self {
            queries: trace.drain_dns_lookups(),
            tcp_connect: trace.drain_tcp_connects(),
            tls_handshakes: trace.drain_tls_handshakes(),
            network_events: trace.drain_network_events(),
            requests: trace.drain_http_requests(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("invalid config: {0}")]
    Parse(#[from] ParseError),

    #[error("serialisation failed: {0}")]
    Serialise(#[from] serde_json::Error),
}

pub async fn run(config: &str) -> Result<String, RunError> {
    let cfg = ExperimentConfig::from_json(config)?;

    let trace = Trace::new(1);
    let start_time = SystemTime::now();
    let start = Instant::now();

    let _ = Experiment::new(cfg.clone(), trace.clone()).run().await;

    let test_keys = TestKeys::from_trace(&trace);
    // TODO: populate resolver asn and resolver ip correctly
    let measurement = Measurement {
        annotations: Default::default(),
        data_format_version: "0.2.0".into(),
        extensions: None,
        input: cfg.input,
        measurement_start_time: start_time,
        probe_asn: cfg.probe_config.probe_asn,
        probe_cc: cfg.probe_config.probe_cc,
        probe_ip: cfg.probe_config.probe_ip,
        probe_network_name: cfg.probe_config.probe_network_name,
        resolver_asn: String::new(),
        resolver_ip: String::new(),
        resolver_network_name: String::new(),
        software_name: cfg.probe_config.software_name,
        software_version: cfg.probe_config.software_version,
        test_helpers: serde_json::Value::Object(Default::default()),
        test_keys: serde_json::to_value(test_keys).unwrap_or_default(),
        test_name: cfg.name,
        test_runtime: start.elapsed().as_secs_f64(),
        test_start_time: start_time,
        test_version: "0.1.0".into(),
    };

    Ok(serde_json::to_string(&measurement)?)
}
