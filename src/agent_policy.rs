//! Multi-level agent policy framework.
//!
//! Provides per-agent tool policies with inheritance:
//!
//! ```text
//! Global → Group → Agent → Subagent → Profile
//! ```
//!
//! Each level can:
//! - Allow specific tools
//! - Deny specific tools
//! - Require asking for specific tools
//! - Set model-aware restrictions
//!
//! ## Usage
//!
//! ```ignore
//! use brainpro::agent_policy::{AgentPolicy, PolicyStack, ToolRestriction};
//!
//! let mut stack = PolicyStack::new();
//!
//! // Global: deny dangerous tools
//! stack.add_global_deny("Bash(rm -rf:*)");
//!
//! // Group: engineering team can run tests
//! stack.add_group_policy("engineering", AgentPolicy::new()
//!     .allow("Bash(cargo test:*)")
//!     .allow("Bash(npm test:*)"));
//!
//! // Agent-specific: this agent can only read files
//! stack.add_agent_policy("reader-bot", AgentPolicy::new()
//!     .allow_only(vec!["Read", "Glob", "Grep"]));
//!
//! // Check permission
//! let decision = stack.resolve("reader-bot", "Read", &args);
//! ```

use crate::config::PermissionMode;
use crate::policy::Decision;
use crate::tool_filter;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Model-aware tool restriction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRestriction {
    /// Pattern for model names (e.g., "gpt-*", "claude-*")
    pub model_pattern: String,
    /// Tools that are denied for this model
    pub denied_tools: Vec<String>,
    /// Reason for the restriction
    pub reason: String,
}

impl ModelRestriction {
    pub fn new(pattern: &str, denied: Vec<&str>, reason: &str) -> Self {
        Self {
            model_pattern: pattern.to_string(),
            denied_tools: denied.into_iter().map(String::from).collect(),
            reason: reason.to_string(),
        }
    }

    /// Check if this restriction applies to the given model
    pub fn applies_to_model(&self, model: &str) -> bool {
        if self.model_pattern.ends_with('*') {
            let prefix = &self.model_pattern[..self.model_pattern.len() - 1];
            model.starts_with(prefix)
        } else {
            model == self.model_pattern
        }
    }

    /// Check if a tool is denied for this model
    pub fn denies_tool(&self, tool: &str) -> bool {
        self.denied_tools.iter().any(|t| t == tool)
    }
}

/// Policy for a single agent or level
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentPolicy {
    /// Tools explicitly allowed
    #[serde(default)]
    pub allow: Vec<String>,

    /// Tools requiring user confirmation
    #[serde(default)]
    pub ask: Vec<String>,

    /// Tools explicitly denied
    #[serde(default)]
    pub deny: Vec<String>,

    /// If set, only these tools are available (allowlist mode)
    #[serde(default)]
    pub allow_only: Option<Vec<String>>,

    /// Model-specific restrictions
    #[serde(default)]
    pub model_restrictions: Vec<ModelRestriction>,

    /// Permission mode override
    #[serde(default)]
    pub mode: Option<PermissionMode>,

    /// Inherit from parent policy (default: true)
    #[serde(default = "default_true")]
    pub inherit: bool,
}

fn default_true() -> bool {
    true
}

impl AgentPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an allow rule
    pub fn allow(mut self, pattern: &str) -> Self {
        self.allow.push(pattern.to_string());
        self
    }

    /// Add an ask rule
    pub fn ask(mut self, pattern: &str) -> Self {
        self.ask.push(pattern.to_string());
        self
    }

    /// Add a deny rule
    pub fn deny(mut self, pattern: &str) -> Self {
        self.deny.push(pattern.to_string());
        self
    }

    /// Set allowlist mode (only these tools available)
    pub fn allow_only(mut self, tools: Vec<&str>) -> Self {
        self.allow_only = Some(tools.into_iter().map(String::from).collect());
        self
    }

    /// Add a model restriction
    pub fn with_model_restriction(mut self, restriction: ModelRestriction) -> Self {
        self.model_restrictions.push(restriction);
        self
    }

    /// Set permission mode override
    pub fn with_mode(mut self, mode: PermissionMode) -> Self {
        self.mode = Some(mode);
        self
    }

    /// Disable inheritance from parent
    pub fn no_inherit(mut self) -> Self {
        self.inherit = false;
        self
    }

    /// Check if a tool is allowed by the allowlist (if set)
    fn check_allowlist(&self, tool: &str) -> Option<bool> {
        self.allow_only.as_ref().map(|allowed| {
            allowed.iter().any(|t| t == tool)
        })
    }

    /// Check model restrictions
    fn check_model_restrictions(&self, tool: &str, model: Option<&str>) -> Option<(Decision, String)> {
        if let Some(model) = model {
            for restriction in &self.model_restrictions {
                if restriction.applies_to_model(model) && restriction.denies_tool(tool) {
                    return Some((Decision::Deny, restriction.reason.clone()));
                }
            }
        }
        None
    }
}

/// Policy level for inheritance
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PolicyLevel {
    Global = 0,
    Group = 1,
    Agent = 2,
    Subagent = 3,
    Profile = 4,
}

impl std::fmt::Display for PolicyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyLevel::Global => write!(f, "global"),
            PolicyLevel::Group => write!(f, "group"),
            PolicyLevel::Agent => write!(f, "agent"),
            PolicyLevel::Subagent => write!(f, "subagent"),
            PolicyLevel::Profile => write!(f, "profile"),
        }
    }
}

/// A policy applied at a specific level
#[derive(Debug, Clone)]
struct LeveledPolicy {
    level: PolicyLevel,
    name: Option<String>, // e.g., group name, agent id
    policy: AgentPolicy,
}

/// Multi-level policy stack
#[derive(Debug, Clone, Default)]
pub struct PolicyStack {
    /// Policies in order from lowest to highest priority
    policies: Vec<LeveledPolicy>,
    /// Group memberships: agent_id -> group_names
    group_memberships: HashMap<String, Vec<String>>,
    /// Current model being used (for model-aware restrictions)
    current_model: Option<String>,
}

impl PolicyStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current model for model-aware restrictions
    pub fn set_model(&mut self, model: &str) {
        self.current_model = Some(model.to_string());
    }

    /// Add a global policy rule (applies to all agents)
    pub fn add_global_policy(&mut self, policy: AgentPolicy) {
        self.policies.push(LeveledPolicy {
            level: PolicyLevel::Global,
            name: None,
            policy,
        });
        self.sort_policies();
    }

    /// Add a global deny pattern
    pub fn add_global_deny(&mut self, pattern: &str) {
        self.add_global_policy(AgentPolicy::new().deny(pattern));
    }

    /// Add a group policy
    pub fn add_group_policy(&mut self, group_name: &str, policy: AgentPolicy) {
        self.policies.push(LeveledPolicy {
            level: PolicyLevel::Group,
            name: Some(group_name.to_string()),
            policy,
        });
        self.sort_policies();
    }

    /// Add an agent to a group
    pub fn add_agent_to_group(&mut self, agent_id: &str, group_name: &str) {
        self.group_memberships
            .entry(agent_id.to_string())
            .or_default()
            .push(group_name.to_string());
    }

    /// Add an agent-specific policy
    pub fn add_agent_policy(&mut self, agent_id: &str, policy: AgentPolicy) {
        self.policies.push(LeveledPolicy {
            level: PolicyLevel::Agent,
            name: Some(agent_id.to_string()),
            policy,
        });
        self.sort_policies();
    }

    /// Add a subagent policy (higher priority than agent)
    pub fn add_subagent_policy(&mut self, subagent_id: &str, policy: AgentPolicy) {
        self.policies.push(LeveledPolicy {
            level: PolicyLevel::Subagent,
            name: Some(subagent_id.to_string()),
            policy,
        });
        self.sort_policies();
    }

    /// Add a profile policy (highest priority, temporary)
    pub fn add_profile_policy(&mut self, profile_name: &str, policy: AgentPolicy) {
        self.policies.push(LeveledPolicy {
            level: PolicyLevel::Profile,
            name: Some(profile_name.to_string()),
            policy,
        });
        self.sort_policies();
    }

    /// Remove profile policies (called when profile is deactivated)
    pub fn clear_profile_policies(&mut self) {
        self.policies.retain(|p| p.level != PolicyLevel::Profile);
    }

    fn sort_policies(&mut self) {
        // Sort by level (lower level = lower priority = processed first)
        self.policies.sort_by_key(|p| p.level);
    }

    /// Get applicable policies for an agent (respecting inheritance)
    fn get_applicable_policies(&self, agent_id: Option<&str>) -> Vec<&LeveledPolicy> {
        let mut applicable = Vec::new();
        let groups = agent_id.and_then(|id| self.group_memberships.get(id));

        for leveled in &self.policies {
            let matches = match leveled.level {
                PolicyLevel::Global => true,
                PolicyLevel::Group => {
                    groups.map_or(false, |g| {
                        leveled.name.as_ref().map_or(false, |n| g.contains(n))
                    })
                }
                PolicyLevel::Agent => {
                    agent_id.map_or(false, |id| {
                        leveled.name.as_ref().map_or(false, |n| n == id)
                    })
                }
                PolicyLevel::Subagent => {
                    // Subagent policies match if agent_id contains the subagent prefix
                    agent_id.map_or(false, |id| {
                        leveled.name.as_ref().map_or(false, |n| id.starts_with(n))
                    })
                }
                PolicyLevel::Profile => true, // Profiles always apply when active
            };

            if matches {
                applicable.push(leveled);
            }
        }

        applicable
    }

    /// Resolve the effective tool policy for an agent and tool
    ///
    /// Returns (Decision, matched_rule, matched_level)
    pub fn resolve(
        &self,
        agent_id: Option<&str>,
        tool: &str,
        args: &Value,
    ) -> (Decision, Option<String>, Option<PolicyLevel>) {
        let arg = extract_tool_arg(tool, args);
        let arg_ref = arg.as_deref();
        let applicable = self.get_applicable_policies(agent_id);

        // Check from highest priority to lowest, but respect inheritance
        for leveled in applicable.iter().rev() {
            let policy = &leveled.policy;

            // Check allowlist first (most restrictive)
            if let Some(allowed) = policy.check_allowlist(tool) {
                if !allowed {
                    return (
                        Decision::Deny,
                        Some(format!("not in allow_only list")),
                        Some(leveled.level),
                    );
                }
            }

            // Check model restrictions
            if let Some((decision, reason)) = policy.check_model_restrictions(tool, self.current_model.as_deref()) {
                return (decision, Some(reason), Some(leveled.level));
            }

            // Check deny rules (highest priority within policy)
            for pattern in &policy.deny {
                if tool_filter::tool_matches(tool, pattern, arg_ref) {
                    return (
                        Decision::Deny,
                        Some(pattern.clone()),
                        Some(leveled.level),
                    );
                }
            }

            // Check ask rules
            for pattern in &policy.ask {
                if tool_filter::tool_matches(tool, pattern, arg_ref) {
                    return (
                        Decision::Ask,
                        Some(pattern.clone()),
                        Some(leveled.level),
                    );
                }
            }

            // Check allow rules
            for pattern in &policy.allow {
                if tool_filter::tool_matches(tool, pattern, arg_ref) {
                    return (
                        Decision::Allow,
                        Some(pattern.clone()),
                        Some(leveled.level),
                    );
                }
            }

            // If this policy doesn't inherit, stop here
            if !policy.inherit {
                break;
            }
        }

        // No explicit rule found, return default
        (Decision::Ask, None, None)
    }

    /// Get the effective permission mode for an agent
    pub fn effective_mode(&self, agent_id: Option<&str>) -> PermissionMode {
        let applicable = self.get_applicable_policies(agent_id);

        // Return the highest priority mode override
        for leveled in applicable.iter().rev() {
            if let Some(mode) = leveled.policy.mode {
                return mode;
            }
            if !leveled.policy.inherit {
                break;
            }
        }

        PermissionMode::Default
    }

    /// Filter tool schemas based on agent policies
    pub fn filter_tools(&self, agent_id: Option<&str>, schemas: Vec<Value>) -> Vec<Value> {
        let applicable = self.get_applicable_policies(agent_id);

        // Collect all allow_only lists (intersection)
        let mut allowlist: Option<Vec<String>> = None;
        for leveled in &applicable {
            if let Some(only) = &leveled.policy.allow_only {
                match &mut allowlist {
                    None => allowlist = Some(only.clone()),
                    Some(existing) => {
                        existing.retain(|t| only.contains(t));
                    }
                }
            }
        }

        schemas
            .into_iter()
            .filter(|schema| {
                let name = schema
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str());

                if let Some(name) = name {
                    // If allowlist is set, only include tools in the list
                    if let Some(ref allowed) = allowlist {
                        return allowed.iter().any(|a| a == name);
                    }

                    // Check for explicit denies
                    for leveled in &applicable {
                        for pattern in &leveled.policy.deny {
                            if tool_filter::tool_matches(name, pattern, None) {
                                return false;
                            }
                        }
                    }

                    // Check model restrictions
                    if let Some(model) = &self.current_model {
                        for leveled in &applicable {
                            for restriction in &leveled.policy.model_restrictions {
                                if restriction.applies_to_model(model) && restriction.denies_tool(name) {
                                    return false;
                                }
                            }
                        }
                    }

                    true
                } else {
                    false
                }
            })
            .collect()
    }

    /// Check if a specific tool is allowed for an agent
    pub fn is_tool_allowed(&self, agent_id: Option<&str>, tool: &str, args: &Value) -> bool {
        let (decision, _, _) = self.resolve(agent_id, tool, args);
        matches!(decision, Decision::Allow)
    }
}

/// Extract the primary argument for rule matching
fn extract_tool_arg(tool: &str, args: &Value) -> Option<String> {
    match tool {
        "Bash" => args
            .get("command")
            .and_then(|v| v.as_str())
            .map(String::from),
        "Write" | "Edit" | "Read" => args.get("path").and_then(|v| v.as_str()).map(String::from),
        "Grep" | "Glob" | "Search" => args
            .get("pattern")
            .and_then(|v| v.as_str())
            .map(String::from),
        _ => None,
    }
}

/// Common model restrictions
pub mod restrictions {
    use super::ModelRestriction;

    /// OpenAI models cannot use ApplyPatch (no file system access)
    pub fn openai_no_apply_patch() -> ModelRestriction {
        ModelRestriction::new(
            "gpt-*",
            vec!["ApplyPatch"],
            "OpenAI models do not support ApplyPatch tool",
        )
    }

    /// Claude models have restrictions on certain tools
    pub fn claude_restrictions() -> ModelRestriction {
        ModelRestriction::new(
            "claude-*",
            vec!["ApplyPatch"],
            "Claude models have limited ApplyPatch support",
        )
    }

    /// Local models (Ollama) should not access external URLs
    pub fn local_no_web() -> ModelRestriction {
        ModelRestriction::new(
            "llama*",
            vec!["WebFetch", "WebSearch"],
            "Local models should not access external URLs",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_agent_policy_builder() {
        let policy = AgentPolicy::new()
            .allow("Read")
            .allow("Glob")
            .deny("Bash(rm:*)")
            .ask("Write");

        assert_eq!(policy.allow.len(), 2);
        assert_eq!(policy.deny.len(), 1);
        assert_eq!(policy.ask.len(), 1);
    }

    #[test]
    fn test_policy_stack_global() {
        let mut stack = PolicyStack::new();
        stack.add_global_deny("Bash(rm -rf:*)");

        let (decision, rule, level) =
            stack.resolve(None, "Bash", &json!({"command": "rm -rf /"}));

        assert_eq!(decision, Decision::Deny);
        assert!(rule.is_some());
        assert_eq!(level, Some(PolicyLevel::Global));
    }

    #[test]
    fn test_policy_stack_agent_override() {
        let mut stack = PolicyStack::new();

        // Global: deny rm
        stack.add_global_deny("Bash(rm:*)");

        // Agent: allow rm for cleanup-agent
        stack.add_agent_policy("cleanup-agent", AgentPolicy::new().allow("Bash(rm:*)"));

        // Other agents still denied
        let (decision, _, _) = stack.resolve(Some("other-agent"), "Bash", &json!({"command": "rm foo.txt"}));
        assert_eq!(decision, Decision::Deny);

        // cleanup-agent is allowed
        let (decision, _, level) = stack.resolve(Some("cleanup-agent"), "Bash", &json!({"command": "rm foo.txt"}));
        assert_eq!(decision, Decision::Allow);
        assert_eq!(level, Some(PolicyLevel::Agent));
    }

    #[test]
    fn test_policy_stack_allowlist() {
        let mut stack = PolicyStack::new();
        stack.add_agent_policy(
            "reader-bot",
            AgentPolicy::new().allow_only(vec!["Read", "Glob", "Grep"]),
        );

        // Read is allowed
        let (decision, _, _) = stack.resolve(Some("reader-bot"), "Read", &json!({}));
        assert_eq!(decision, Decision::Ask); // Ask because no explicit allow

        // Write is denied (not in allowlist)
        let (decision, _, _) = stack.resolve(Some("reader-bot"), "Write", &json!({}));
        assert_eq!(decision, Decision::Deny);
    }

    #[test]
    fn test_model_restriction() {
        let restriction = ModelRestriction::new(
            "gpt-*",
            vec!["ApplyPatch"],
            "Not supported",
        );

        assert!(restriction.applies_to_model("gpt-4"));
        assert!(restriction.applies_to_model("gpt-4o-mini"));
        assert!(!restriction.applies_to_model("claude-3"));

        assert!(restriction.denies_tool("ApplyPatch"));
        assert!(!restriction.denies_tool("Read"));
    }

    #[test]
    fn test_policy_stack_model_restrictions() {
        let mut stack = PolicyStack::new();
        stack.set_model("gpt-4o");
        stack.add_global_policy(
            AgentPolicy::new()
                .with_model_restriction(restrictions::openai_no_apply_patch()),
        );

        let (decision, reason, _) = stack.resolve(None, "ApplyPatch", &json!({}));
        assert_eq!(decision, Decision::Deny);
        assert!(reason.unwrap().contains("OpenAI"));
    }

    #[test]
    fn test_group_policies() {
        let mut stack = PolicyStack::new();

        // Engineering group can run tests
        stack.add_group_policy(
            "engineering",
            AgentPolicy::new()
                .allow("Bash(cargo test:*)")
                .allow("Bash(npm test:*)"),
        );

        // Add agent to group
        stack.add_agent_to_group("dev-bot", "engineering");

        // dev-bot can run tests
        let (decision, _, level) = stack.resolve(
            Some("dev-bot"),
            "Bash",
            &json!({"command": "cargo test"}),
        );
        assert_eq!(decision, Decision::Allow);
        assert_eq!(level, Some(PolicyLevel::Group));

        // other-bot cannot
        let (decision, _, _) = stack.resolve(
            Some("other-bot"),
            "Bash",
            &json!({"command": "cargo test"}),
        );
        assert_eq!(decision, Decision::Ask);
    }

    #[test]
    fn test_no_inherit() {
        let mut stack = PolicyStack::new();

        // Global: allow Read
        stack.add_global_policy(AgentPolicy::new().allow("Read"));

        // Agent: no inherit, only Glob allowed
        stack.add_agent_policy(
            "isolated-bot",
            AgentPolicy::new().allow("Glob").no_inherit(),
        );

        // Glob is allowed
        let (decision, _, _) = stack.resolve(Some("isolated-bot"), "Glob", &json!({}));
        assert_eq!(decision, Decision::Allow);

        // Read would be allowed by global, but inherit is disabled
        let (decision, _, _) = stack.resolve(Some("isolated-bot"), "Read", &json!({}));
        assert_eq!(decision, Decision::Ask); // Falls through to default
    }

    #[test]
    fn test_filter_tools() {
        let mut stack = PolicyStack::new();
        stack.add_agent_policy(
            "reader-bot",
            AgentPolicy::new().allow_only(vec!["Read", "Glob"]),
        );

        let schemas = vec![
            json!({"function": {"name": "Read"}}),
            json!({"function": {"name": "Write"}}),
            json!({"function": {"name": "Glob"}}),
            json!({"function": {"name": "Bash"}}),
        ];

        let filtered = stack.filter_tools(Some("reader-bot"), schemas);
        assert_eq!(filtered.len(), 2);

        let names: Vec<&str> = filtered
            .iter()
            .filter_map(|s| s["function"]["name"].as_str())
            .collect();
        assert!(names.contains(&"Read"));
        assert!(names.contains(&"Glob"));
        assert!(!names.contains(&"Write"));
    }
}
