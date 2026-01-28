//! Agent loop and execution components.
//!
//! This module provides the core agent loop functionality:
//! - `CommandStats` - Token and tool usage statistics
//! - `TurnResult` - Result of a single agent turn
//! - `PendingQuestion` - Question awaiting user input
//! - `run_turn` / `run_turn_sync` - Agent turn execution (backward compat)
//! - `core` - Shared agent loop with hooks
//! - `tool_executor` - Shared tool execution with policy/hooks

pub mod core;
pub mod tool_executor;

// Re-export the tool executor for worker.rs
pub use tool_executor::execute_simple;

// Re-export core loop for persona implementations
pub use core::run_loop;

// Re-export types from agent_impl (the original agent.rs, renamed)
// These are kept for backward compatibility with cli.rs, subagent.rs, etc.
pub use crate::agent_impl::{run_turn, run_turn_sync, CommandStats, PendingQuestion, TurnResult};
