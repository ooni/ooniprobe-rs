//! Measurement trace.
//!
//! A [`Trace`] is the central object for a single "step" of a measurement.
//!
//! Every network primitive (DNS, TCP, TLS, HTTP) receives a reference to
//! the trace.  When an operation completes (or fails) the primitive calls
//! one of the `record_*` methods, which push an archival event into a
//! bounded channel inside the trace.  The experiment then drains those
//! channels at whatever cadence it chooses.

use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc, Mutex,
};
use std::time::Instant;

use crate::archival::{
    DnsLookupResult, HttpTransaction, NetworkEvent, TcpConnectResult, TlsHandshakeResult,
};

// Buffer sizes matching Go's measurexlite constants.
const DNS_LOOKUP_BUF: usize = 8;
const TCP_CONNECT_BUF: usize = 8;
const TLS_HANDSHAKE_BUF: usize = 8;
const NETWORK_EVENT_BUF: usize = 64;
const HTTP_REQUEST_BUF: usize = 8;

/// TraceInner - Inner shared state
struct TraceInner {
    /// Monotonic start instant for relative timing.
    zero_time: Instant,

    index: i64,
    tx_counter: AtomicI64,
    tags: Vec<String>,

    dns_lookups: Mutex<Vec<DnsLookupResult>>,
    tcp_connects: Mutex<Vec<TcpConnectResult>>,
    tls_handshakes: Mutex<Vec<TlsHandshakeResult>>,
    network_events: Mutex<Vec<NetworkEvent>>,
    http_requests: Mutex<Vec<HttpTransaction>>,
}

/// A measurement trace.
#[derive(Clone)]
pub struct Trace(Arc<TraceInner>);

impl Trace {
    /// Create a new trace with the given index.
    ///
    /// `index` is used to form transaction IDs and should be unique
    /// across all traces within a single measurement run.
    pub fn new(index: i64) -> Self {
        Self(Arc::new(TraceInner {
            zero_time: Instant::now(),
            index,
            tx_counter: AtomicI64::new(0),
            tags: vec![],
            dns_lookups: Mutex::new(Vec::with_capacity(DNS_LOOKUP_BUF)),
            tcp_connects: Mutex::new(Vec::with_capacity(TCP_CONNECT_BUF)),
            tls_handshakes: Mutex::new(Vec::with_capacity(TLS_HANDSHAKE_BUF)),
            network_events: Mutex::new(Vec::with_capacity(NETWORK_EVENT_BUF)),
            http_requests: Mutex::new(Vec::with_capacity(HTTP_REQUEST_BUF)),
        }))
    }

    /// Create a new trace with custom tags attached to every observation.
    pub fn new_with_tags(index: i64, tags: Vec<String>) -> Self {
        Self(Arc::new(TraceInner {
            zero_time: Instant::now(),
            index,
            tx_counter: AtomicI64::new(0),
            tags,
            dns_lookups: Mutex::new(Vec::with_capacity(DNS_LOOKUP_BUF)),
            tcp_connects: Mutex::new(Vec::with_capacity(TCP_CONNECT_BUF)),
            tls_handshakes: Mutex::new(Vec::with_capacity(TLS_HANDSHAKE_BUF)),
            network_events: Mutex::new(Vec::with_capacity(NETWORK_EVENT_BUF)),
            http_requests: Mutex::new(Vec::with_capacity(HTTP_REQUEST_BUF)),
        }))
    }

    // Timing

    /// Elapsed time since the trace was created, as seconds (for `t0`/`t` fields).
    pub fn elapsed_secs(&self) -> f64 {
        self.0.zero_time.elapsed().as_secs_f64()
    }

    /// Return the `Instant` at which measurement began (zero-time).
    pub fn zero_time(&self) -> Instant {
        self.0.zero_time
    }

    /// Convenience: duration from zero-time to a given instant, in seconds.
    pub fn secs_since(&self, t: Instant) -> f64 {
        t.saturating_duration_since(self.0.zero_time).as_secs_f64()
    }

    // Transaction IDs

    /// Allocate the next unique transaction ID for this trace.
    pub fn next_transaction_id(&self) -> i64 {
        let n = self.0.tx_counter.fetch_add(1, Ordering::Relaxed);
        self.0.index * 10_000 + n
    }

    /// Return the trace index.
    pub fn index(&self) -> i64 {
        self.0.index
    }

    // Tags

    /// Tags that are automatically appended to every recorded observation.
    pub fn tags(&self) -> &[String] {
        &self.0.tags
    }

    // Record

    /// Record a DNS lookup result.
    pub fn record_dns_lookup(&self, mut obs: DnsLookupResult) {
        obs.tags
            .get_or_insert_with(Vec::new)
            .extend_from_slice(&self.0.tags);
        let mut guard = self.0.dns_lookups.lock().unwrap();
        if guard.len() < DNS_LOOKUP_BUF * 4 {
            guard.push(obs);
        }
    }

    /// Record a TCP connect result.
    pub fn record_tcp_connect(&self, mut obs: TcpConnectResult) {
        obs.tags
            .get_or_insert_with(Vec::new)
            .extend_from_slice(&self.0.tags);
        let mut guard = self.0.tcp_connects.lock().unwrap();
        if guard.len() < TCP_CONNECT_BUF * 4 {
            guard.push(obs);
        }
    }

    /// Record a TLS (or QUIC) handshake result.
    pub fn record_tls_handshake(&self, mut obs: TlsHandshakeResult) {
        obs.tags
            .get_or_insert_with(Vec::new)
            .extend_from_slice(&self.0.tags);
        let mut guard = self.0.tls_handshakes.lock().unwrap();
        if guard.len() < TLS_HANDSHAKE_BUF * 4 {
            guard.push(obs);
        }
    }

    /// Record a low-level network I/O event.
    pub fn record_network_event(&self, mut obs: NetworkEvent) {
        obs.tags
            .get_or_insert_with(Vec::new)
            .extend_from_slice(&self.0.tags);
        let mut guard = self.0.network_events.lock().unwrap();
        if guard.len() < NETWORK_EVENT_BUF * 4 {
            guard.push(obs);
        }
    }

    /// Record an HTTP request/response pair.
    pub fn record_http_request(&self, mut obs: HttpTransaction) {
        obs.tags
            .get_or_insert_with(Vec::new)
            .extend_from_slice(&self.0.tags);
        let mut guard = self.0.http_requests.lock().unwrap();
        if guard.len() < HTTP_REQUEST_BUF * 4 {
            guard.push(obs);
        }
    }

    // Drain

    pub fn drain_dns_lookups(&self) -> Vec<DnsLookupResult> {
        let mut guard = self.0.dns_lookups.lock().unwrap();
        std::mem::take(&mut *guard)
    }

    pub fn drain_tcp_connects(&self) -> Vec<TcpConnectResult> {
        let mut guard = self.0.tcp_connects.lock().unwrap();
        std::mem::take(&mut *guard)
    }

    pub fn drain_tls_handshakes(&self) -> Vec<TlsHandshakeResult> {
        let mut guard = self.0.tls_handshakes.lock().unwrap();
        std::mem::take(&mut *guard)
    }

    pub fn drain_network_events(&self) -> Vec<NetworkEvent> {
        let mut guard = self.0.network_events.lock().unwrap();
        std::mem::take(&mut *guard)
    }

    pub fn drain_http_requests(&self) -> Vec<HttpTransaction> {
        let mut guard = self.0.http_requests.lock().unwrap();
        std::mem::take(&mut *guard)
    }

    // Introspect (non-draining)

    /// Snapshot of DNS lookups without draining.
    pub fn dns_lookups(&self) -> Vec<DnsLookupResult> {
        self.0.dns_lookups.lock().unwrap().clone()
    }

    /// Snapshot of TCP connects without draining.
    pub fn tcp_connects(&self) -> Vec<TcpConnectResult> {
        self.0.tcp_connects.lock().unwrap().clone()
    }

    /// Snapshot of TLS handshakes without draining.
    pub fn tls_handshakes(&self) -> Vec<TlsHandshakeResult> {
        self.0.tls_handshakes.lock().unwrap().clone()
    }

    /// Snapshot of network events without draining.
    pub fn network_events(&self) -> Vec<NetworkEvent> {
        self.0.network_events.lock().unwrap().clone()
    }
}

/// A span within a trace — represents the timing window of one logical
/// operation (a single TCP connect, one TLS handshake, etc.).
pub struct Span {
    pub t0: f64,
    pub t: f64,
    zero_time: Instant,
    start: Instant,
}

impl Span {
    /// Open a new span, recording `t0`.
    pub fn start(trace: &Trace) -> Self {
        let start = Instant::now();
        Self {
            t0: trace.secs_since(start),
            t: 0.0,
            zero_time: trace.zero_time(),
            start,
        }
    }

    /// Close the span, recording `t`.
    pub fn finish(&mut self) {
        let now = Instant::now();
        self.t = now.saturating_duration_since(self.zero_time).as_secs_f64();
    }
}
