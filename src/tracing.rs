use serde::{Deserialize, Serialize};

use quanta::Instant;
pub enum TracingError {
    GenericError,
    SpanAlreadyEntered,
    SpanNotEntered,
    SpanAlreadyExited,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct NetworkEvent {
    address: Option<String>,
    failure: Option<String>,
    num_bytes: Option<u32>,
    operation: Option<String>,
    proto: Option<String>,
    transaction_id: Option<u32>,
    t0: Option<f64>,
    t: Option<f64>,
}

#[derive(Clone)]
pub struct NetworkEventCollector {
    measurement_start_time: Instant,
    current_span_idx: u32,
    spans: Vec<NetworkEventSpan>,
}

impl<'a> NetworkEventCollector {
    pub fn new(measurement_start_time: Instant) -> Self {
        Self {
            measurement_start_time,
            current_span_idx: 5000,
            spans: vec![],
        }
    }

    pub fn new_span(&mut self) -> NetworkEventSpan {
        let span = NetworkEventSpan::new(self.new_span_idx(), self.measurement_start_time.clone());
        span
    }

    pub fn print_network_events(&mut self) {
        for span in &self.spans {
            match serde_json::to_string(&span.network_event) {
                Ok(json) => println!("{}", json),
                Err(e) => eprintln!("Error serializing span: {}", e),
            }
        }
    }

    pub fn collect_span(&mut self, network_event_span: NetworkEventSpan) {
        self.spans.push(network_event_span)
    }

    fn new_span_idx(&mut self) -> u32 {
        self.current_span_idx += 1;
        self.current_span_idx
    }
}

#[derive(Clone)]
pub struct NetworkEventSpan {
    measurement_start_time: Instant,
    network_event: NetworkEvent,
}

impl NetworkEventSpan {
    fn new(transaction_id: u32, measurement_start_time: Instant) -> Self {
        let mut network_event = NetworkEvent::default();
        network_event.transaction_id = Some(transaction_id);
        Self {
            measurement_start_time,
            network_event,
        }
    }

    pub fn set_address(&mut self, value: &str) -> Result<(), TracingError> {
        self.network_event.address = Some(value.to_string());
        Ok(())
    }

    pub fn set_failure(&mut self, value: &str) -> Result<(), TracingError> {
        self.network_event.failure = Some(value.to_string());
        Ok(())
    }
    pub fn set_operation(&mut self, value: &str) -> Result<(), TracingError> {
        self.network_event.operation = Some(value.to_string());
        Ok(())
    }
    pub fn set_proto(&mut self, value: &str) -> Result<(), TracingError> {
        self.network_event.proto = Some(value.to_string());
        Ok(())
    }

    pub fn set_num_bytes(&mut self, value: u32) -> Result<(), TracingError> {
        self.network_event.num_bytes = Some(value);
        Ok(())
    }

    pub fn enter(&mut self) -> Result<(), TracingError> {
        if self.network_event.t0.is_some() {
            return Err(TracingError::SpanAlreadyEntered);
        }
        self.network_event.t0 = Some(self.measurement_start_time.elapsed().as_secs_f64());
        Ok(())
    }

    pub fn exit(&mut self) -> Result<(), TracingError> {
        if self.network_event.t0.is_none() {
            return Err(TracingError::SpanNotEntered);
        }
        if self.network_event.t.is_some() {
            return Err(TracingError::SpanAlreadyExited);
        }
        self.network_event.t = Some(self.measurement_start_time.elapsed().as_secs_f64());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    #[test]
    fn test_tracing() {
        let measurement_start_time = quanta::Instant::now();
        let mut collector = NetworkEventCollector::new(measurement_start_time);
        let mut span = collector.new_span();
        span.enter();
        span.set_address("1.1.1.1:443");
        thread::sleep(Duration::from_secs(1));
        span.exit();
        collector.collect_span(span);
        collector.print_network_events();
    }
}
