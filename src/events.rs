//! Rich event streaming system for observability.
//!
//! Provides a first-class event streaming architecture similar to production
//! agent systems. Events are structured, typed, and support external propagation.
//!
//! ## Event Categories
//!
//! - **Model**: LLM interactions (usage, errors, streaming)
//! - **Message**: User/assistant message lifecycle
//! - **Session**: Session state changes
//! - **Tool**: Tool invocation and results
//! - **Queue**: Request queuing and lane status
//! - **Run**: Agent turn attempts and completions
//! - **System**: Heartbeat, reload, health
//!
//! ## Usage
//!
//! ```ignore
//! use brainpro::events::{EventBus, Event, EventType};
//!
//! let bus = EventBus::new();
//! bus.subscribe(|event| {
//!     println!("[{}] {:?}", event.subsystem, event.event_type);
//! });
//! bus.emit(Event::model_usage("claude", "claude-3-5-sonnet", 1000, 500, 0.05));
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Monotonically increasing sequence counter for event ordering
static EVENT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn next_sequence() -> u64 {
    EVENT_SEQUENCE.fetch_add(1, Ordering::SeqCst)
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Subsystem identifiers for event categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subsystem {
    /// LLM model interactions
    Model,
    /// Message lifecycle
    Message,
    /// Session management
    Session,
    /// Tool execution
    Tool,
    /// Request queue
    Queue,
    /// Agent run/turn
    Run,
    /// System health
    System,
    /// Circuit breaker
    Circuit,
    /// Policy decisions
    Policy,
    /// Webhook delivery
    Webhook,
    /// Plugin lifecycle
    Plugin,
    /// Cost tracking
    Cost,
}

impl std::fmt::Display for Subsystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Subsystem::Model => "model",
            Subsystem::Message => "message",
            Subsystem::Session => "session",
            Subsystem::Tool => "tool",
            Subsystem::Queue => "queue",
            Subsystem::Run => "run",
            Subsystem::System => "system",
            Subsystem::Circuit => "circuit",
            Subsystem::Policy => "policy",
            Subsystem::Webhook => "webhook",
            Subsystem::Plugin => "plugin",
            Subsystem::Cost => "cost",
        };
        write!(f, "{}", s)
    }
}

/// Event types for the streaming system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventType {
    // Model events
    ModelUsage {
        backend: String,
        model: String,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
        duration_ms: u64,
    },
    ModelError {
        backend: String,
        model: String,
        error_code: String,
        error_message: String,
    },
    ModelStreamStart {
        backend: String,
        model: String,
        request_id: String,
    },
    ModelStreamEnd {
        backend: String,
        model: String,
        request_id: String,
        total_tokens: u64,
    },

    // Message events
    MessageQueued {
        session_id: String,
        message_id: String,
        role: String,
    },
    MessageProcessing {
        session_id: String,
        message_id: String,
    },
    MessageComplete {
        session_id: String,
        message_id: String,
        duration_ms: u64,
    },

    // Session events
    SessionCreated {
        session_id: String,
        agent_id: Option<String>,
    },
    SessionResumed {
        session_id: String,
    },
    SessionPaused {
        session_id: String,
        reason: String,
    },
    SessionEnded {
        session_id: String,
        total_turns: u32,
        total_cost_usd: f64,
    },
    SessionStuck {
        session_id: String,
        stuck_reason: String,
        stuck_tool: Option<String>,
    },

    // Tool events
    ToolInvoked {
        session_id: String,
        tool_name: String,
        tool_call_id: String,
        args_preview: String,
    },
    ToolCompleted {
        session_id: String,
        tool_name: String,
        tool_call_id: String,
        success: bool,
        duration_ms: u64,
    },
    ToolTimeout {
        session_id: String,
        tool_name: String,
        tool_call_id: String,
        timeout_ms: u64,
    },
    ToolDenied {
        session_id: String,
        tool_name: String,
        reason: String,
        policy_rule: Option<String>,
    },

    // Queue events
    QueueLaneCreated {
        lane: String,
        priority: u32,
    },
    QueueLaneActive {
        lane: String,
        pending_count: u32,
    },
    QueueRequestEnqueued {
        lane: String,
        request_id: String,
        position: u32,
    },
    QueueRequestDequeued {
        lane: String,
        request_id: String,
        wait_time_ms: u64,
    },

    // Run events
    RunAttempt {
        session_id: String,
        turn_number: u32,
        iteration: u32,
    },
    RunComplete {
        session_id: String,
        turn_number: u32,
        iterations: u32,
        tool_uses: u32,
        tokens_used: u64,
    },
    RunDoomLoopDetected {
        session_id: String,
        turn_number: u32,
        tool_name: String,
        repeat_count: u32,
    },

    // System events
    Heartbeat {
        uptime_secs: u64,
        active_sessions: u32,
        pending_requests: u32,
    },
    ConfigReload {
        changed_keys: Vec<String>,
    },
    HealthCheck {
        status: String,
        backends: HashMap<String, String>,
    },

    // Circuit breaker events
    CircuitOpened {
        backend: String,
        failure_count: u32,
        recovery_timeout_secs: u32,
    },
    CircuitHalfOpen {
        backend: String,
        probes_remaining: u32,
    },
    CircuitClosed {
        backend: String,
        success_probes: u32,
    },

    // Policy events
    PolicyDecision {
        tool_name: String,
        decision: String,
        rule: Option<String>,
        agent_id: Option<String>,
    },
    PolicyViolation {
        tool_name: String,
        violation: String,
        agent_id: Option<String>,
    },

    // Webhook events
    WebhookDeliveryStarted {
        webhook_id: String,
        url: String,
        event_type: String,
    },
    WebhookDeliveryCompleted {
        webhook_id: String,
        status_code: u16,
        duration_ms: u64,
    },
    WebhookDeliveryFailed {
        webhook_id: String,
        error: String,
        will_retry: bool,
    },

    // Cost events
    CostThresholdWarning {
        session_id: String,
        current_cost_usd: f64,
        threshold_usd: f64,
    },
    CostBudgetExceeded {
        session_id: String,
        budget_usd: f64,
        actual_usd: f64,
    },
}

/// A single event with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Monotonic sequence number for ordering
    pub seq: u64,
    /// Timestamp in milliseconds since epoch
    pub timestamp_ms: u64,
    /// Subsystem that generated this event
    pub subsystem: Subsystem,
    /// The event data
    #[serde(flatten)]
    pub event_type: EventType,
    /// Optional run context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_context: Option<RunContext>,
}

/// Context for associating events with a specific run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunContext {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_number: Option<u32>,
}

impl Event {
    /// Create a new event
    pub fn new(subsystem: Subsystem, event_type: EventType) -> Self {
        Self {
            seq: next_sequence(),
            timestamp_ms: timestamp_ms(),
            subsystem,
            event_type,
            run_context: None,
        }
    }

    /// Create event with run context
    pub fn with_context(
        subsystem: Subsystem,
        event_type: EventType,
        session_id: &str,
        agent_id: Option<&str>,
        turn_number: Option<u32>,
    ) -> Self {
        Self {
            seq: next_sequence(),
            timestamp_ms: timestamp_ms(),
            subsystem,
            event_type,
            run_context: Some(RunContext {
                session_id: session_id.to_string(),
                agent_id: agent_id.map(String::from),
                turn_number,
            }),
        }
    }

    // Convenience constructors for common events

    pub fn model_usage(
        backend: &str,
        model: &str,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
        duration_ms: u64,
    ) -> Self {
        Self::new(
            Subsystem::Model,
            EventType::ModelUsage {
                backend: backend.to_string(),
                model: model.to_string(),
                input_tokens,
                output_tokens,
                cost_usd,
                duration_ms,
            },
        )
    }

    pub fn model_error(backend: &str, model: &str, error_code: &str, error_message: &str) -> Self {
        Self::new(
            Subsystem::Model,
            EventType::ModelError {
                backend: backend.to_string(),
                model: model.to_string(),
                error_code: error_code.to_string(),
                error_message: error_message.to_string(),
            },
        )
    }

    pub fn tool_invoked(
        session_id: &str,
        tool_name: &str,
        tool_call_id: &str,
        args_preview: &str,
    ) -> Self {
        Self::new(
            Subsystem::Tool,
            EventType::ToolInvoked {
                session_id: session_id.to_string(),
                tool_name: tool_name.to_string(),
                tool_call_id: tool_call_id.to_string(),
                args_preview: args_preview.to_string(),
            },
        )
    }

    pub fn tool_completed(
        session_id: &str,
        tool_name: &str,
        tool_call_id: &str,
        success: bool,
        duration_ms: u64,
    ) -> Self {
        Self::new(
            Subsystem::Tool,
            EventType::ToolCompleted {
                session_id: session_id.to_string(),
                tool_name: tool_name.to_string(),
                tool_call_id: tool_call_id.to_string(),
                success,
                duration_ms,
            },
        )
    }

    pub fn tool_denied(
        session_id: &str,
        tool_name: &str,
        reason: &str,
        policy_rule: Option<&str>,
    ) -> Self {
        Self::new(
            Subsystem::Tool,
            EventType::ToolDenied {
                session_id: session_id.to_string(),
                tool_name: tool_name.to_string(),
                reason: reason.to_string(),
                policy_rule: policy_rule.map(String::from),
            },
        )
    }

    pub fn session_created(session_id: &str, agent_id: Option<&str>) -> Self {
        Self::new(
            Subsystem::Session,
            EventType::SessionCreated {
                session_id: session_id.to_string(),
                agent_id: agent_id.map(String::from),
            },
        )
    }

    pub fn session_ended(session_id: &str, total_turns: u32, total_cost_usd: f64) -> Self {
        Self::new(
            Subsystem::Session,
            EventType::SessionEnded {
                session_id: session_id.to_string(),
                total_turns,
                total_cost_usd,
            },
        )
    }

    pub fn session_stuck(session_id: &str, stuck_reason: &str, stuck_tool: Option<&str>) -> Self {
        Self::new(
            Subsystem::Session,
            EventType::SessionStuck {
                session_id: session_id.to_string(),
                stuck_reason: stuck_reason.to_string(),
                stuck_tool: stuck_tool.map(String::from),
            },
        )
    }

    pub fn run_attempt(session_id: &str, turn_number: u32, iteration: u32) -> Self {
        Self::new(
            Subsystem::Run,
            EventType::RunAttempt {
                session_id: session_id.to_string(),
                turn_number,
                iteration,
            },
        )
    }

    pub fn run_complete(
        session_id: &str,
        turn_number: u32,
        iterations: u32,
        tool_uses: u32,
        tokens_used: u64,
    ) -> Self {
        Self::new(
            Subsystem::Run,
            EventType::RunComplete {
                session_id: session_id.to_string(),
                turn_number,
                iterations,
                tool_uses,
                tokens_used,
            },
        )
    }

    pub fn run_doom_loop(
        session_id: &str,
        turn_number: u32,
        tool_name: &str,
        repeat_count: u32,
    ) -> Self {
        Self::new(
            Subsystem::Run,
            EventType::RunDoomLoopDetected {
                session_id: session_id.to_string(),
                turn_number,
                tool_name: tool_name.to_string(),
                repeat_count,
            },
        )
    }

    pub fn circuit_opened(backend: &str, failure_count: u32, recovery_timeout_secs: u32) -> Self {
        Self::new(
            Subsystem::Circuit,
            EventType::CircuitOpened {
                backend: backend.to_string(),
                failure_count,
                recovery_timeout_secs,
            },
        )
    }

    pub fn circuit_closed(backend: &str, success_probes: u32) -> Self {
        Self::new(
            Subsystem::Circuit,
            EventType::CircuitClosed {
                backend: backend.to_string(),
                success_probes,
            },
        )
    }

    pub fn policy_decision(
        tool_name: &str,
        decision: &str,
        rule: Option<&str>,
        agent_id: Option<&str>,
    ) -> Self {
        Self::new(
            Subsystem::Policy,
            EventType::PolicyDecision {
                tool_name: tool_name.to_string(),
                decision: decision.to_string(),
                rule: rule.map(String::from),
                agent_id: agent_id.map(String::from),
            },
        )
    }

    pub fn heartbeat(uptime_secs: u64, active_sessions: u32, pending_requests: u32) -> Self {
        Self::new(
            Subsystem::System,
            EventType::Heartbeat {
                uptime_secs,
                active_sessions,
                pending_requests,
            },
        )
    }

    pub fn cost_threshold_warning(
        session_id: &str,
        current_cost_usd: f64,
        threshold_usd: f64,
    ) -> Self {
        Self::new(
            Subsystem::Cost,
            EventType::CostThresholdWarning {
                session_id: session_id.to_string(),
                current_cost_usd,
                threshold_usd,
            },
        )
    }
}

/// Event listener callback type
pub type EventListener = Arc<dyn Fn(&Event) + Send + Sync>;

/// Event bus for pub/sub event distribution
pub struct EventBus {
    listeners: RwLock<Vec<EventListener>>,
    /// Filter by subsystem (None = all)
    subsystem_filters: RwLock<HashMap<usize, Vec<Subsystem>>>,
}

impl EventBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            listeners: RwLock::new(Vec::new()),
            subsystem_filters: RwLock::new(HashMap::new()),
        })
    }

    /// Subscribe to all events
    pub fn subscribe<F>(&self, listener: F) -> usize
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        let mut listeners = self.listeners.write().unwrap();
        let id = listeners.len();
        listeners.push(Arc::new(listener));
        id
    }

    /// Subscribe to specific subsystems only
    pub fn subscribe_filtered<F>(&self, subsystems: Vec<Subsystem>, listener: F) -> usize
    where
        F: Fn(&Event) + Send + Sync + 'static,
    {
        let id = self.subscribe(listener);
        let mut filters = self.subsystem_filters.write().unwrap();
        filters.insert(id, subsystems);
        id
    }

    /// Emit an event to all subscribers
    pub fn emit(&self, event: Event) {
        let listeners = self.listeners.read().unwrap();
        let filters = self.subsystem_filters.read().unwrap();

        for (id, listener) in listeners.iter().enumerate() {
            // Check subsystem filter
            if let Some(allowed) = filters.get(&id) {
                if !allowed.contains(&event.subsystem) {
                    continue;
                }
            }

            // Note: We catch panics in listeners to prevent one bad listener
            // from breaking the entire event system. This is intentional -
            // events are observability, not control flow.
            let listener = Arc::clone(listener);
            let event_clone = event.clone();
            std::thread::spawn(move || {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    listener(&event_clone);
                }));
            });
        }
    }

    /// Emit synchronously (blocks until all listeners complete)
    pub fn emit_sync(&self, event: Event) {
        let listeners = self.listeners.read().unwrap();
        let filters = self.subsystem_filters.read().unwrap();

        for (id, listener) in listeners.iter().enumerate() {
            if let Some(allowed) = filters.get(&id) {
                if !allowed.contains(&event.subsystem) {
                    continue;
                }
            }

            // Catch panics but don't spawn threads
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                listener(&event);
            }));
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self {
            listeners: RwLock::new(Vec::new()),
            subsystem_filters: RwLock::new(HashMap::new()),
        }
    }
}

/// Global event bus singleton
static GLOBAL_BUS: std::sync::OnceLock<Arc<EventBus>> = std::sync::OnceLock::new();

/// Get the global event bus
pub fn global_bus() -> &'static Arc<EventBus> {
    GLOBAL_BUS.get_or_init(EventBus::new)
}

/// Emit an event to the global bus
pub fn emit(event: Event) {
    global_bus().emit(event);
}

/// Emit synchronously to the global bus
pub fn emit_sync(event: Event) {
    global_bus().emit_sync(event);
}

/// Subscribe to the global bus
pub fn subscribe<F>(listener: F) -> usize
where
    F: Fn(&Event) + Send + Sync + 'static,
{
    global_bus().subscribe(listener)
}

/// Subscribe to specific subsystems on the global bus
pub fn subscribe_filtered<F>(subsystems: Vec<Subsystem>, listener: F) -> usize
where
    F: Fn(&Event) + Send + Sync + 'static,
{
    global_bus().subscribe_filtered(subsystems, listener)
}

/// Subsystem logger for scoped event emission
pub struct SubsystemLogger {
    subsystem: Subsystem,
    session_id: Option<String>,
    agent_id: Option<String>,
}

impl SubsystemLogger {
    pub fn new(subsystem: Subsystem) -> Self {
        Self {
            subsystem,
            session_id: None,
            agent_id: None,
        }
    }

    pub fn with_session(mut self, session_id: &str) -> Self {
        self.session_id = Some(session_id.to_string());
        self
    }

    pub fn with_agent(mut self, agent_id: &str) -> Self {
        self.agent_id = Some(agent_id.to_string());
        self
    }

    pub fn emit(&self, event_type: EventType) {
        let event = if let Some(session_id) = &self.session_id {
            Event::with_context(
                self.subsystem,
                event_type,
                session_id,
                self.agent_id.as_deref(),
                None,
            )
        } else {
            Event::new(self.subsystem, event_type)
        };
        emit(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_event_sequence() {
        let e1 = Event::model_usage("claude", "claude-3-5-sonnet", 100, 50, 0.01, 1000);
        let e2 = Event::model_usage("claude", "claude-3-5-sonnet", 100, 50, 0.01, 1000);
        assert!(e2.seq > e1.seq);
    }

    #[test]
    fn test_event_serialization() {
        let event = Event::tool_invoked("session-1", "Read", "call-1", "path=src/main.rs");
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("tool_invoked"));
        assert!(json.contains("session-1"));
    }

    #[test]
    fn test_event_bus_subscribe() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        bus.subscribe(move |_event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

        bus.emit_sync(Event::heartbeat(100, 1, 0));

        // Give async a moment
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subsystem_filter() {
        let bus = EventBus::new();
        let tool_counter = Arc::new(AtomicUsize::new(0));
        let model_counter = Arc::new(AtomicUsize::new(0));

        let tc = Arc::clone(&tool_counter);
        bus.subscribe_filtered(vec![Subsystem::Tool], move |_| {
            tc.fetch_add(1, Ordering::SeqCst);
        });

        let mc = Arc::clone(&model_counter);
        bus.subscribe_filtered(vec![Subsystem::Model], move |_| {
            mc.fetch_add(1, Ordering::SeqCst);
        });

        bus.emit_sync(Event::tool_invoked("s1", "Read", "c1", "preview"));
        bus.emit_sync(Event::model_usage("claude", "model", 100, 50, 0.01, 500));

        std::thread::sleep(std::time::Duration::from_millis(10));
        assert_eq!(tool_counter.load(Ordering::SeqCst), 1);
        assert_eq!(model_counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_subsystem_logger() {
        let logger = SubsystemLogger::new(Subsystem::Tool)
            .with_session("test-session")
            .with_agent("test-agent");

        // Just verify it doesn't panic
        logger.emit(EventType::ToolInvoked {
            session_id: "test-session".to_string(),
            tool_name: "Read".to_string(),
            tool_call_id: "call-1".to_string(),
            args_preview: "preview".to_string(),
        });
    }
}
