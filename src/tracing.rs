use std::sync::Arc;

use quanta::Instant;
pub enum TracingError {
    GenericError,
}

#[derive(Default, Clone)]
pub struct NetworkEventMetadata {
    address: Option<String>,
    failure: Option<String>,
    num_bytes: Option<u32>,
    operation: Option<String>,
    proto: Option<String>,
}

#[derive(Default, Clone)]
pub struct NetworkEvent {
    metadata: NetworkEventMetadata,
    t0: Option<f64>,
    t: Option<f64>,
}

pub struct NetworkEventCollector {
    measurement_start_time: Instant,
    current_span_idx: u32,
}

impl NetworkEventCollector {
    pub fn new(measurement_start_time: Instant) -> Self {
        Self {
            measurement_start_time,
            current_span_idx: 5000,
        }
    }

    pub fn new_span(&mut self) -> NetworkEventSpan {
        let span = NetworkEventSpan::new(self.new_span_idx(), self.measurement_start_time);
        span
    }

    pub fn new_span_idx(&mut self) -> u32 {
        self.current_span_idx += 1;
        self.current_span_idx
    }
}

pub struct NetworkEventSpan {
    transaction_id: u32,
    network_event: Arc<Option<NetworkEvent>>,
    measurement_start_time: Instant,
}

impl NetworkEventSpan {
    pub fn new(transaction_id: u32, measurement_start_time: Instant) -> Self {
        Self {
            transaction_id,
            measurement_start_time,
            network_event: Arc::new(None),
        }
    }

    pub fn enter(&mut self) -> Arc<Option<NetworkEvent>> {
        let ne = NetworkEvent {
            metadata: NetworkEventMetadata::default(),
            t0: Some(self.measurement_start_time.elapsed().as_secs_f64()),
            t: None,
        };
        self.network_event = Arc::new(Some(ne));
        self.network_event.clone()
    }

    pub fn exit(&mut self) -> Result<(), TracingError> {
        // TODO: in here we should be appending to a list of spans inside of the
        // NetworkEventCollector.
        match Arc::get_mut(&mut self.network_event) {
            Some(Some(ne)) => {
                ne.t = Some(self.measurement_start_time.elapsed().as_secs_f64());
                Ok(())
            }
            _ => Err(TracingError::GenericError),
        }
    }
}
