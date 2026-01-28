//! Observability metrics for LLM operations.
//!
//! Provides Prometheus-compatible metrics and JSON export for:
//! - Request counts by backend/model/status
//! - Latency histograms
//! - Circuit breaker trips
//! - Token usage
//! - Cost tracking

use prometheus::{CounterVec, Encoder, HistogramOpts, HistogramVec, Opts, Registry, TextEncoder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Labels for backend metrics
const BACKEND_LABEL: &str = "backend";
const MODEL_LABEL: &str = "model";
const STATUS_LABEL: &str = "status";
const DIRECTION_LABEL: &str = "direction";

/// Metrics collector for LLM operations
pub struct MetricsCollector {
    registry: Registry,

    /// Total requests by backend, model, status
    requests_total: CounterVec,

    /// Request duration in milliseconds
    requests_duration_ms: HistogramVec,

    /// Circuit breaker trips by backend
    circuit_trips_total: CounterVec,

    /// Total tokens by backend, model, direction (input/output)
    tokens_total: CounterVec,

    /// Total cost in USD by backend, model
    cost_usd_total: CounterVec,

    /// JSON export data (accumulated)
    json_data: Arc<RwLock<MetricsSnapshot>>,
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        let registry = Registry::new();

        // Requests counter
        let requests_opts = Opts::new("brainpro_requests_total", "Total LLM requests");
        let requests_total =
            CounterVec::new(requests_opts, &[BACKEND_LABEL, MODEL_LABEL, STATUS_LABEL])
                .expect("Failed to create requests counter");
        registry
            .register(Box::new(requests_total.clone()))
            .expect("Failed to register requests counter");

        // Duration histogram
        let duration_opts = HistogramOpts::new(
            "brainpro_requests_duration_ms",
            "Request duration in milliseconds",
        )
        .buckets(vec![
            100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 30000.0, 60000.0,
        ]);
        let requests_duration_ms =
            HistogramVec::new(duration_opts, &[BACKEND_LABEL, MODEL_LABEL])
                .expect("Failed to create duration histogram");
        registry
            .register(Box::new(requests_duration_ms.clone()))
            .expect("Failed to register duration histogram");

        // Circuit trips counter
        let circuit_opts = Opts::new(
            "brainpro_circuit_trips_total",
            "Total circuit breaker trips",
        );
        let circuit_trips_total =
            CounterVec::new(circuit_opts, &[BACKEND_LABEL]).expect("Failed to create circuit counter");
        registry
            .register(Box::new(circuit_trips_total.clone()))
            .expect("Failed to register circuit counter");

        // Tokens counter
        let tokens_opts = Opts::new("brainpro_tokens_total", "Total tokens processed");
        let tokens_total =
            CounterVec::new(tokens_opts, &[BACKEND_LABEL, MODEL_LABEL, DIRECTION_LABEL])
                .expect("Failed to create tokens counter");
        registry
            .register(Box::new(tokens_total.clone()))
            .expect("Failed to register tokens counter");

        // Cost counter
        let cost_opts = Opts::new("brainpro_cost_usd_total", "Total cost in USD");
        let cost_usd_total =
            CounterVec::new(cost_opts, &[BACKEND_LABEL, MODEL_LABEL])
                .expect("Failed to create cost counter");
        registry
            .register(Box::new(cost_usd_total.clone()))
            .expect("Failed to register cost counter");

        Self {
            registry,
            requests_total,
            requests_duration_ms,
            circuit_trips_total,
            tokens_total,
            cost_usd_total,
            json_data: Arc::new(RwLock::new(MetricsSnapshot::default())),
        }
    }

    /// Record a successful request
    pub fn record_request_success(&self, backend: &str, model: &str, duration_ms: u64) {
        self.requests_total
            .with_label_values(&[backend, model, "success"])
            .inc();
        self.requests_duration_ms
            .with_label_values(&[backend, model])
            .observe(duration_ms as f64);

        // Update JSON data
        let mut data = self.json_data.write().unwrap();
        data.total_requests += 1;
        data.successful_requests += 1;
        *data.requests_by_backend.entry(backend.to_string()).or_default() += 1;
        *data.requests_by_model.entry(model.to_string()).or_default() += 1;
    }

    /// Record a successful request with full token/cost data
    ///
    /// Also emits an event via the events module for richer observability.
    pub fn record_request_success_full(
        &self,
        backend: &str,
        model: &str,
        duration_ms: u64,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
    ) {
        self.record_request_success(backend, model, duration_ms);
        self.record_tokens(backend, model, input_tokens, output_tokens);
        self.record_cost(backend, model, cost_usd);
        // Event emission is done by caller if needed
    }

    /// Record a failed request
    pub fn record_request_failure(&self, backend: &str, model: &str, duration_ms: u64) {
        self.requests_total
            .with_label_values(&[backend, model, "failure"])
            .inc();
        self.requests_duration_ms
            .with_label_values(&[backend, model])
            .observe(duration_ms as f64);

        // Update JSON data
        let mut data = self.json_data.write().unwrap();
        data.total_requests += 1;
        data.failed_requests += 1;
        *data.requests_by_backend.entry(backend.to_string()).or_default() += 1;
    }

    /// Record a failed request with error details
    ///
    /// Also emits an event via the events module for richer observability.
    pub fn record_request_failure_with_error(
        &self,
        backend: &str,
        model: &str,
        duration_ms: u64,
        _error_code: &str,
        _error_message: &str,
    ) {
        self.record_request_failure(backend, model, duration_ms);
        // Event emission is done by caller if needed
    }

    /// Record a circuit breaker trip
    pub fn record_circuit_trip(&self, backend: &str) {
        self.circuit_trips_total
            .with_label_values(&[backend])
            .inc();

        // Update JSON data
        let mut data = self.json_data.write().unwrap();
        data.circuit_trips += 1;
        *data.circuit_trips_by_backend.entry(backend.to_string()).or_default() += 1;
    }

    /// Record a circuit breaker trip with details
    ///
    /// Also emits an event via the events module for richer observability.
    pub fn record_circuit_trip_with_details(
        &self,
        backend: &str,
        _failure_count: u32,
        _recovery_timeout_secs: u32,
    ) {
        self.record_circuit_trip(backend);
        // Event emission is done by caller if needed
    }

    /// Record token usage
    pub fn record_tokens(
        &self,
        backend: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
    ) {
        self.tokens_total
            .with_label_values(&[backend, model, "input"])
            .inc_by(input_tokens as f64);
        self.tokens_total
            .with_label_values(&[backend, model, "output"])
            .inc_by(output_tokens as f64);

        // Update JSON data
        let mut data = self.json_data.write().unwrap();
        data.total_input_tokens += input_tokens;
        data.total_output_tokens += output_tokens;
    }

    /// Record cost
    pub fn record_cost(&self, backend: &str, model: &str, cost_usd: f64) {
        self.cost_usd_total
            .with_label_values(&[backend, model])
            .inc_by(cost_usd);

        // Update JSON data
        let mut data = self.json_data.write().unwrap();
        data.total_cost_usd += cost_usd;
        *data.cost_by_model.entry(model.to_string()).or_default() += cost_usd;
    }

    /// Get Prometheus-formatted metrics
    pub fn prometheus_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .expect("Failed to encode metrics");
        String::from_utf8(buffer).expect("Metrics should be valid UTF-8")
    }

    /// Get current metrics snapshot as JSON
    pub fn json_snapshot(&self) -> MetricsSnapshot {
        self.json_data.read().unwrap().clone()
    }

    /// Export metrics to JSON file
    pub fn export_to_json(&self, path: &PathBuf) -> anyhow::Result<()> {
        let snapshot = self.json_snapshot();
        let json = serde_json::to_string_pretty(&snapshot)?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, json)?;
        Ok(())
    }

    /// Export to default location (~/.brainpro/metrics.json)
    pub fn export_to_default_location(&self) -> anyhow::Result<()> {
        if let Some(home) = dirs::home_dir() {
            let path = home.join(".brainpro").join("metrics.json");
            self.export_to_json(&path)
        } else {
            Err(anyhow::anyhow!("Cannot determine home directory"))
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Snapshot of metrics for JSON export
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    /// Timestamp when snapshot was taken
    #[serde(default)]
    pub timestamp: u64,

    /// Total requests
    pub total_requests: u64,
    /// Successful requests
    pub successful_requests: u64,
    /// Failed requests
    pub failed_requests: u64,

    /// Requests by backend
    pub requests_by_backend: HashMap<String, u64>,
    /// Requests by model
    pub requests_by_model: HashMap<String, u64>,

    /// Total input tokens
    pub total_input_tokens: u64,
    /// Total output tokens
    pub total_output_tokens: u64,

    /// Total cost in USD
    pub total_cost_usd: f64,
    /// Cost by model
    pub cost_by_model: HashMap<String, f64>,

    /// Circuit breaker trips
    pub circuit_trips: u64,
    /// Circuit trips by backend
    pub circuit_trips_by_backend: HashMap<String, u64>,
}

impl MetricsSnapshot {
    /// Add timestamp to snapshot
    pub fn with_timestamp(mut self) -> Self {
        self.timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self
    }
}

/// Global metrics instance
static METRICS: std::sync::OnceLock<MetricsCollector> = std::sync::OnceLock::new();

/// Get the global metrics collector
pub fn global() -> &'static MetricsCollector {
    METRICS.get_or_init(MetricsCollector::new)
}

/// Record a successful request to global metrics
pub fn record_success(backend: &str, model: &str, duration_ms: u64) {
    global().record_request_success(backend, model, duration_ms);
}

/// Record a failed request to global metrics
pub fn record_failure(backend: &str, model: &str, duration_ms: u64) {
    global().record_request_failure(backend, model, duration_ms);
}

/// Record token usage to global metrics
pub fn record_tokens(backend: &str, model: &str, input: u64, output: u64) {
    global().record_tokens(backend, model, input, output);
}

/// Record cost to global metrics
pub fn record_cost(backend: &str, model: &str, cost_usd: f64) {
    global().record_cost(backend, model, cost_usd);
}

/// Record a circuit trip to global metrics
pub fn record_circuit_trip(backend: &str) {
    global().record_circuit_trip(backend);
}

/// Get Prometheus metrics from global collector
pub fn prometheus() -> String {
    global().prometheus_metrics()
}

/// Get JSON snapshot from global collector
pub fn snapshot() -> MetricsSnapshot {
    global().json_snapshot().with_timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector() {
        let collector = MetricsCollector::new();

        // Record some metrics
        collector.record_request_success("claude", "claude-3-5-sonnet", 1500);
        collector.record_request_success("claude", "claude-3-5-sonnet", 2000);
        collector.record_request_failure("chatgpt", "gpt-4o", 500);
        collector.record_tokens("claude", "claude-3-5-sonnet", 1000, 500);
        collector.record_cost("claude", "claude-3-5-sonnet", 0.05);
        collector.record_circuit_trip("chatgpt");

        // Check JSON snapshot
        let snapshot = collector.json_snapshot();
        assert_eq!(snapshot.total_requests, 3);
        assert_eq!(snapshot.successful_requests, 2);
        assert_eq!(snapshot.failed_requests, 1);
        assert_eq!(snapshot.total_input_tokens, 1000);
        assert_eq!(snapshot.total_output_tokens, 500);
        assert_eq!(snapshot.circuit_trips, 1);
        assert!((snapshot.total_cost_usd - 0.05).abs() < 0.001);

        // Check prometheus output
        let prom = collector.prometheus_metrics();
        assert!(prom.contains("brainpro_requests_total"));
        assert!(prom.contains("brainpro_tokens_total"));
    }

    #[test]
    fn test_snapshot_timestamp() {
        let snapshot = MetricsSnapshot::default().with_timestamp();
        assert!(snapshot.timestamp > 0);
    }
}
