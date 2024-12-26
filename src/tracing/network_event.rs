use serde::{Deserialize, Serialize};

use quanta::Instant;
pub enum TracingError {
    GenericError,
    SpanAlreadyEntered,
    SpanNotEntered,
    SpanAlreadyExited,
}

#[derive(Clone, Serialize, Debug)]
pub struct NetworkEvent {
    pub address: Option<String>,
    pub failure: Option<String>,
    pub num_bytes: Option<u32>,
    pub operation: Option<String>,
    pub proto: Option<String>,
    pub transaction_id: u32,
    pub t0: Option<f64>,
    pub t: Option<f64>,

    #[serde(skip_serializing)]
    measurement_start_time: quanta::Instant,
}

impl NetworkEvent {
    pub fn set_address(&mut self, value: &str) -> Result<(), TracingError> {
        self.address = Some(value.to_string());
        Ok(())
    }

    pub fn set_failure(&mut self, value: &str) -> Result<(), TracingError> {
        self.failure = Some(value.to_string());
        Ok(())
    }
    pub fn set_operation(&mut self, value: &str) -> Result<(), TracingError> {
        self.operation = Some(value.to_string());
        Ok(())
    }
    pub fn set_proto(&mut self, value: &str) -> Result<(), TracingError> {
        self.proto = Some(value.to_string());
        Ok(())
    }

    pub fn set_num_bytes(&mut self, value: u32) -> Result<(), TracingError> {
        self.num_bytes = Some(value);
        Ok(())
    }

    pub fn enter(&mut self) -> Result<(), TracingError> {
        if self.t0.is_some() {
            return Err(TracingError::SpanAlreadyEntered);
        }
        self.t0 = Some(self.measurement_start_time.elapsed().as_secs_f64());
        Ok(())
    }

    pub fn exit(&mut self) -> Result<(), TracingError> {
        if self.t0.is_none() {
            return Err(TracingError::SpanNotEntered);
        }
        if self.t.is_some() {
            return Err(TracingError::SpanAlreadyExited);
        }
        self.t = Some(self.measurement_start_time.elapsed().as_secs_f64());
        Ok(())
    }
}

#[derive(Clone)]
pub struct NetworkEventCollector {
    measurement_start_time: Instant,
    current_transaction_idx: u32,
    transactions: Vec<NetworkEventTransaction>,
}

impl<'a> NetworkEventCollector {
    pub fn new(measurement_start_time: Instant) -> Self {
        Self {
            measurement_start_time,
            current_transaction_idx: 5000,
            transactions: vec![],
        }
    }

    pub fn new_transaction(&mut self) -> NetworkEventTransaction {
        let span = NetworkEventTransaction::new(
            self.new_transaction_idx(),
            self.measurement_start_time.clone(),
        );
        span
    }

    pub fn print_network_events(&mut self) {
        for span in &self.transactions {
            match serde_json::to_string(&span.network_events) {
                Ok(json) => println!("{}", json),
                Err(e) => eprintln!("Error serializing span: {}", e),
            }
        }
    }

    pub fn collect_transaction(&mut self, network_event_transaction: NetworkEventTransaction) {
        self.transactions.push(network_event_transaction)
    }

    fn new_transaction_idx(&mut self) -> u32 {
        self.current_transaction_idx += 1;
        self.current_transaction_idx
    }
}

#[derive(Clone, Debug)]
pub struct NetworkEventTransaction {
    measurement_start_time: Instant,
    transaction_id: u32,
    network_events: Vec<NetworkEvent>,
}

impl NetworkEventTransaction {
    fn new(transaction_id: u32, measurement_start_time: Instant) -> Self {
        Self {
            measurement_start_time,
            transaction_id,
            network_events: vec![],
        }
    }

    pub fn new_network_event(&self) -> NetworkEvent {
        NetworkEvent {
            measurement_start_time: self.measurement_start_time,
            address: None,
            failure: None,
            num_bytes: None,
            operation: None,
            proto: None,
            t0: None,
            t: None,
            transaction_id: self.transaction_id,
        }
    }

    pub fn collect_network_event(&mut self, network_event: NetworkEvent) {
        self.network_events.push(network_event)
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

        let mut transaction1 = collector.new_transaction();
        let mut network_event1 = transaction1.new_network_event();
        network_event1.enter();
        network_event1.set_address("1.1.1.1:443");
        thread::sleep(Duration::from_micros(42));
        network_event1.exit();
        transaction1.collect_network_event(network_event1);

        let mut network_event2 = transaction1.new_network_event();
        network_event2.enter();
        network_event2.set_address("2.2.2.2:443");
        thread::sleep(Duration::from_micros(100));
        network_event2.exit();
        transaction1.collect_network_event(network_event2);

        collector.collect_transaction(transaction1);

        let mut transaction2 = collector.new_transaction();
        let mut network_event1 = transaction2.new_network_event();
        network_event1.enter();
        network_event1.set_address("1.1.1.1:443");
        thread::sleep(Duration::from_micros(42));
        network_event1.exit();
        transaction2.collect_network_event(network_event1);

        let mut network_event2 = transaction2.new_network_event();
        network_event2.enter();
        network_event2.set_address("2.2.2.2:443");
        thread::sleep(Duration::from_micros(100));
        network_event2.exit();
        transaction2.collect_network_event(network_event2);

        collector.collect_transaction(transaction2);
        collector.print_network_events();
    }
}
