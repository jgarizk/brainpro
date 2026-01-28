//! Core agent loop implementation.
//!
//! This module provides the shared agent loop that can be customized
//! via the AgentHooks trait. This eliminates ~1500 lines of duplicated
//! code across:
//! - agent_impl.rs (run_turn_sync, run_turn)
//! - mrcode/loop_impl.rs
//! - mrbot/loop_impl.rs
//! - worker.rs

use crate::agent::tool_executor::{self, DispatchResult};
use crate::cli::Context;
use crate::llm::{self, LlmClient};
use crate::plan::{self, PlanPhase};
use crate::tool_display;
use anyhow::Result;
use serde_json::{json, Value};
use std::io::{self, Write};

/// Default maximum iterations per turn
pub const DEFAULT_MAX_ITERATIONS: usize = 12;

/// Configuration for the agent loop
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Maximum iterations before stopping
    pub max_iterations: usize,
    /// Whether to include Task tool (subagent delegation)
    pub include_task_tool: bool,
    /// Whether to use streaming for LLM calls
    pub streaming: bool,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: DEFAULT_MAX_ITERATIONS,
            include_task_tool: false,
            streaming: false,
        }
    }
}

impl AgentLoopConfig {
    pub fn with_task_tool(mut self) -> Self {
        self.include_task_tool = true;
        self
    }

    pub fn with_streaming(mut self) -> Self {
        self.streaming = true;
        self
    }

    pub fn with_max_iterations(mut self, n: usize) -> Self {
        self.max_iterations = n;
        self
    }
}

/// Pending question that needs user input before continuing
#[derive(Debug, Clone)]
pub struct PendingQuestion {
    pub tool_call_id: String,
    pub questions: Vec<crate::tools::ask_user::Question>,
}

/// Result of a single agent turn
#[derive(Debug, Default, Clone)]
pub struct TurnResult {
    /// Token and tool usage statistics
    pub stats: crate::agent::CommandStats,
    /// If true, a Stop hook requested continuation with the given prompt
    pub force_continue: bool,
    /// The prompt to use for continuation
    pub continue_prompt: Option<String>,
    /// If set, agent is waiting for user to answer questions
    pub pending_question: Option<PendingQuestion>,
    /// Collected response text from the assistant
    pub response_text: Option<String>,
}

/// Hooks for customizing agent loop behavior.
///
/// Implement this trait to create a custom agent loop with
/// different system prompts, tool filtering, or streaming behavior.
pub trait AgentHooks {
    /// Build the system prompt for this agent.
    ///
    /// This is called at the start of each iteration.
    fn build_system_prompt(&self, ctx: &Context, in_planning_mode: bool) -> String;

    /// Filter or transform tool schemas.
    ///
    /// Called after loading base schemas to allow filtering or modification.
    fn filter_tools(&self, schemas: Vec<Value>, in_planning_mode: bool) -> Vec<Value> {
        if in_planning_mode {
            // Default: only read-only tools in planning mode
            schemas
                .into_iter()
                .filter(|schema| {
                    schema
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .map(|name| matches!(name, "Read" | "Glob" | "Search"))
                        .unwrap_or(false)
                })
                .collect()
        } else {
            schemas
        }
    }

    /// Called when streaming content is received.
    ///
    /// Default implementation prints to stdout.
    fn on_stream_content(&self, content: &str) {
        print!("{}", content);
        let _ = io::stdout().flush();
    }

    /// Called when non-streaming content is received.
    ///
    /// Default implementation prints to stdout with newline.
    fn on_content(&self, content: &str) {
        println!("{}", content);
    }
}

/// Trace helper
fn trace(ctx: &Context, label: &str, content: &str) {
    if *ctx.tracing.borrow() {
        eprintln!("[TRACE:{}] {}", label, content);
    }
}

/// Verbose helper
fn verbose(ctx: &Context, message: &str) {
    if ctx.args.verbose || ctx.args.debug {
        eprintln!("[VERBOSE] {}", message);
    }
}

/// Auto-activate skills mentioned with $skill-name syntax
fn auto_activate_skills(ctx: &Context, user_input: &str) {
    for word in user_input.split_whitespace() {
        if word.starts_with('$') && word.len() > 1 {
            let skill_name =
                &word[1..].trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-');
            let index = ctx.skill_index.borrow();
            if index.get(skill_name).is_some() {
                let active = ctx.active_skills.borrow();
                if active.get(skill_name).is_none() {
                    drop(active);
                    let mut active = ctx.active_skills.borrow_mut();
                    if let Ok(activation) = active.activate(skill_name, &index) {
                        let _ = ctx.transcript.borrow_mut().skill_activate(
                            &activation.name,
                            Some("auto-activated from $mention"),
                            activation.allowed_tools.as_ref(),
                        );
                        trace(ctx, "SKILL", &format!("Auto-activated: {}", skill_name));
                    }
                }
            }
        }
    }
}

/// Apply skill-based tool filtering
fn apply_skill_tool_filter(ctx: &Context, mut schemas: Vec<Value>) -> Vec<Value> {
    let active_skills = ctx.active_skills.borrow();
    let effective_allowed = active_skills.effective_allowed_tools();
    drop(active_skills);

    if let Some(allowed) = &effective_allowed {
        schemas.retain(|schema| {
            if let Some(name) = schema
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                // ActivateSkill is always available
                if name == "ActivateSkill" {
                    return true;
                }
                // Task is always available for subagent delegation
                if name == "Task" {
                    return true;
                }
                allowed.iter().any(|a| a == name)
            } else {
                false
            }
        });
    }
    schemas
}

/// Process plan mode output
fn process_plan_output(ctx: &Context, content: &str) {
    let goal = ctx
        .plan_mode
        .borrow()
        .current_plan
        .as_ref()
        .map(|p| p.goal.clone())
        .unwrap_or_default();

    if let Ok(parsed_plan) = plan::parse_plan_output(content, &goal) {
        let mut state = ctx.plan_mode.borrow_mut();
        if let Some(current_plan) = &mut state.current_plan {
            current_plan.summary = parsed_plan.summary;
            current_plan.steps = parsed_plan.steps;
            current_plan.status = plan::PlanStatus::Ready;
        }
        state.enter_review();

        let plan_name = state
            .current_plan
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_default();
        let step_count = state
            .current_plan
            .as_ref()
            .map(|p| p.steps.len())
            .unwrap_or(0);
        drop(state);
        let _ = ctx
            .transcript
            .borrow_mut()
            .plan_created(&plan_name, step_count);
    }
}

/// Run the core agent loop (sync version).
///
/// This is the shared implementation used by all agent variants.
pub fn run_loop<H: AgentHooks>(
    hooks: &H,
    ctx: &Context,
    config: &AgentLoopConfig,
    user_input: &str,
    messages: &mut Vec<Value>,
) -> Result<TurnResult> {
    use crate::tools;

    let mut turn_result = TurnResult::default();
    let mut collected_response = String::new();
    let _ = ctx.transcript.borrow_mut().user_message(user_input);

    messages.push(json!({
        "role": "user",
        "content": user_input
    }));

    // Resolve target
    let target = {
        let current = ctx.current_target.borrow();
        if let Some(t) = current.as_ref() {
            t.clone()
        } else {
            ctx.config
                .borrow()
                .get_default_target()
                .ok_or_else(|| anyhow::anyhow!("No target configured. Use --target or /target"))?
        }
    };
    let bash_config = ctx.config.borrow().bash.clone();

    trace(ctx, "TARGET", &target.to_string());

    // Check plan mode
    let plan_phase = ctx.plan_mode.borrow().phase;
    let in_planning_mode = plan_phase == PlanPhase::Planning;

    // Auto-activate skills from $mentions
    auto_activate_skills(ctx, user_input);

    // Get tool schemas
    let schema_opts = tools::SchemaOptions::new(ctx.args.optimize);
    let base_schemas = if config.include_task_tool {
        tools::schemas_with_task(&schema_opts)
    } else {
        tools::schemas(&schema_opts)
    };

    // Apply hooks filtering
    let filtered_schemas = hooks.filter_tools(base_schemas, in_planning_mode);

    // Apply skill-based filtering
    let tool_schemas = apply_skill_tool_filter(ctx, filtered_schemas);

    // Use configured max_iterations
    let max_iterations = ctx.args.max_turns.unwrap_or(config.max_iterations);

    for iteration in 1..=max_iterations {
        trace(ctx, "ITER", &format!("Starting iteration {}", iteration));

        // Build system prompt via hooks
        let system_prompt = hooks.build_system_prompt(ctx, in_planning_mode);

        // Make LLM request
        let response = {
            let mut backends = ctx.backends.borrow_mut();
            let client = backends.get_client(&target.backend)?;

            let mut req_messages = vec![json!({
                "role": "system",
                "content": system_prompt
            })];
            req_messages.extend(messages.clone());

            let request = llm::ChatRequest {
                model: target.model.clone(),
                messages: req_messages,
                tools: Some(tool_schemas.clone()),
                tool_choice: Some("auto".to_string()),
            };

            client.chat(&request)?
        };

        // Track token usage
        if let Some(usage) = &response.usage {
            turn_result.stats.input_tokens += usage.prompt_tokens;
            turn_result.stats.output_tokens += usage.completion_tokens;

            let turn_number = *ctx.turn_counter.borrow();
            let op = ctx.session_costs.borrow_mut().record_operation(
                turn_number,
                &target.model,
                usage.prompt_tokens,
                usage.completion_tokens,
            );

            let _ = ctx.transcript.borrow_mut().token_usage(
                &target.model,
                usage.prompt_tokens,
                usage.completion_tokens,
                op.cost_usd,
            );
        }

        if response.choices.is_empty() {
            eprintln!("No response from model");
            break;
        }

        let choice = &response.choices[0];
        let msg = &choice.message;

        // Warn if truncated
        if choice.finish_reason.as_deref() == Some("length") {
            eprintln!("⚠️  Response truncated (max tokens reached). Consider increasing max_tokens or using /compact.");
        }

        // Handle content
        if let Some(content) = &msg.content {
            if !content.is_empty() {
                hooks.on_content(content);
                if !collected_response.is_empty() {
                    collected_response.push_str("\n\n");
                }
                collected_response.push_str(content);
                let _ = ctx.transcript.borrow_mut().assistant_message(content);

                if in_planning_mode {
                    process_plan_output(ctx, content);
                }
            }
        }

        // Check for tool calls
        let tool_calls = match &msg.tool_calls {
            Some(tc) if !tc.is_empty() => {
                if let Some(content) = &msg.content {
                    if !content.is_empty() {
                        trace(ctx, "THINK", content);
                    }
                }
                tc
            }
            _ => {
                messages.push(json!({
                    "role": "assistant",
                    "content": msg.content
                }));
                break;
            }
        };

        let assistant_msg = json!({
            "role": "assistant",
            "content": msg.content,
            "tool_calls": tool_calls
        });
        messages.push(assistant_msg);

        // Execute tool calls
        for tc in tool_calls {
            let name = &tc.function.name;
            let args: Value = serde_json::from_str(&tc.function.arguments).unwrap_or(json!({}));

            turn_result.stats.tool_uses += 1;

            trace(
                ctx,
                "CALL",
                &format!(
                    "{}({})",
                    name,
                    serde_json::to_string_pretty(&args).unwrap_or_default()
                ),
            );

            verbose(
                ctx,
                &format!("Tool call: {}({})", name, tc.function.arguments),
            );

            eprintln!("{}", tool_display::format_tool_call(name, &args));
            let _ = ctx.transcript.borrow_mut().tool_call(name, &args);

            // Execute tool with policy/hooks
            let (dispatch_result, ok, _duration_ms) =
                tool_executor::execute_with_policy(ctx, name, args.clone(), &bash_config);

            // Handle dispatch result
            let result = match dispatch_result {
                DispatchResult::Ok(v) | DispatchResult::Error(v) => v,
                DispatchResult::AskUser { result, questions } => {
                    turn_result.pending_question = Some(PendingQuestion {
                        tool_call_id: tc.id.clone(),
                        questions,
                    });
                    result
                }
                DispatchResult::Task { result, stats } => {
                    turn_result.stats.merge(&stats);
                    result
                }
            };

            trace(
                ctx,
                "RESULT",
                &format!(
                    "{}: {}",
                    name,
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                ),
            );

            verbose(ctx, &format!("Tool result: {} ok={}", name, ok));
            eprintln!("{}", tool_display::format_tool_result(name, &result));

            messages.push(json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": serde_json::to_string(&result)?
            }));

            // Break if pending question
            if turn_result.pending_question.is_some() {
                break;
            }
        }

        // Break outer loop if pending question
        if turn_result.pending_question.is_some() {
            break;
        }
    }

    // Run Stop hooks (skip if pending question)
    if turn_result.pending_question.is_none() {
        let last_assistant = messages.iter().rev().find_map(|m| {
            if m["role"].as_str() == Some("assistant") {
                m["content"].as_str().map(|s| s.to_string())
            } else {
                None
            }
        });

        if let Some(content) = last_assistant {
            let (hook_triggered, continue_prompt) = ctx.hooks.borrow().on_stop("tool_finished", Some(&content));
            if hook_triggered {
                turn_result.force_continue = true;
                turn_result.continue_prompt = continue_prompt;
            }
        }
    }

    turn_result.response_text = if collected_response.is_empty() {
        None
    } else {
        Some(collected_response)
    };

    Ok(turn_result)
}

// Note: Streaming version can be added later when the LlmClient streaming API is ready

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_loop_config_default() {
        let config = AgentLoopConfig::default();
        assert_eq!(config.max_iterations, DEFAULT_MAX_ITERATIONS);
        assert!(!config.include_task_tool);
        assert!(!config.streaming);
    }

    #[test]
    fn test_agent_loop_config_builder() {
        let config = AgentLoopConfig::default()
            .with_task_tool()
            .with_streaming()
            .with_max_iterations(5);
        assert_eq!(config.max_iterations, 5);
        assert!(config.include_task_tool);
        assert!(config.streaming);
    }
}
