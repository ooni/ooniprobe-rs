#[cfg(test)]
mod tests {
    use ooniprobe_experiments::measurement::run;
    use std::fs;

    #[tokio::test]
    async fn tcping_success() {
        let config = r#"{
            "experiment": "web_connectivity",
            "input": "example.com:443",
            "probe_config": {
                "probe_asn": "AS0",
                "probe_cc": "ZZ",
                "probe_ip": "127.0.0.1",
                "probe_network_name": "Test Network",
                "software_name": "test",
                "software_version": "0.0.1"
            },
            "steps": [
                {"type": "dns", "hostname": "example.com"},
                {"type": "tcp", "port": 443},
                {"type": "tls"},
                {"type": "http", "url": "https://example.com"}
            ]
        }"#;

        let json = run(config).await.unwrap();

        let m: serde_json::Value = serde_json::from_str(&json).unwrap();
        let pretty = serde_json::to_string_pretty(&m).unwrap();

        fs::write("webconnectivity_measurement.json", &pretty).unwrap();
    }
}
