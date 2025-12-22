//! Model routing for situational model selection.
//!
//! This module enables automatic model selection based on:
//! - Subagent type (inferred from name/description)
//! - Hardcoded defaults with config overrides

use crate::config::Target;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Route categories for model selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteCategory {
    Planning,
    Coding,
    Exploration,
    Testing,
    Documentation,
    Fast,
    Default,
}

impl RouteCategory {
    /// Infer category from agent name/description
    pub fn from_agent_name(name: &str, description: &str) -> Self {
        let combined = format!("{} {}", name, description).to_lowercase();

        if combined.contains("plan")
            || combined.contains("architect")
            || combined.contains("design")
        {
            RouteCategory::Planning
        } else if combined.contains("patch")
            || combined.contains("edit")
            || combined.contains("refactor")
            || combined.contains("code")
            || combined.contains("implement")
        {
            RouteCategory::Coding
        } else if combined.contains("scout")
            || combined.contains("explore")
            || combined.contains("find")
            || combined.contains("search")
        {
            RouteCategory::Exploration
        } else if combined.contains("test")
            || combined.contains("verify")
            || combined.contains("check")
        {
            RouteCategory::Testing
        } else if combined.contains("doc")
            || combined.contains("readme")
            || combined.contains("comment")
        {
            RouteCategory::Documentation
        } else {
            RouteCategory::Default
        }
    }
}

/// Configuration for model routing
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelRoutingConfig {
    #[serde(default)]
    pub routes: HashMap<RouteCategory, String>, // category -> target string
}

/// Hardcoded default routes (sensible defaults)
fn default_routes() -> HashMap<RouteCategory, String> {
    let mut routes = HashMap::new();

    // These are sensible defaults - users can override in config
    // Format: model@backend

    // Planning tasks benefit from strong reasoning
    routes.insert(
        RouteCategory::Planning,
        "qwen3-235b-a22b-instruct-2507@venice".to_string(),
    );

    // Coding needs strong code generation
    routes.insert(
        RouteCategory::Coding,
        "claude-3-5-sonnet-latest@claude".to_string(),
    );

    // Exploration can use faster models
    routes.insert(
        RouteCategory::Exploration,
        "gpt-4o-mini@chatgpt".to_string(),
    );

    // Testing needs reliable execution
    routes.insert(RouteCategory::Testing, "gpt-4o-mini@chatgpt".to_string());

    // Documentation
    routes.insert(
        RouteCategory::Documentation,
        "gpt-4o-mini@chatgpt".to_string(),
    );

    // Fast operations
    routes.insert(RouteCategory::Fast, "gpt-4o-mini@chatgpt".to_string());

    // Default fallback
    routes.insert(RouteCategory::Default, "gpt-4o-mini@chatgpt".to_string());

    routes
}

/// Model router that resolves targets based on context
pub struct ModelRouter {
    config: ModelRoutingConfig,
    defaults: HashMap<RouteCategory, String>,
}

impl ModelRouter {
    pub fn new(config: ModelRoutingConfig) -> Self {
        Self {
            config,
            defaults: default_routes(),
        }
    }

    /// Resolve target for a route category
    pub fn resolve(&self, category: RouteCategory, fallback: &Target) -> Target {
        // Check user config first
        if let Some(target_str) = self.config.routes.get(&category) {
            if let Some(target) = Target::parse(target_str) {
                return target;
            }
        }

        // Check defaults
        if let Some(target_str) = self.defaults.get(&category) {
            if let Some(target) = Target::parse(target_str) {
                return target;
            }
        }

        // Fallback to provided default
        fallback.clone()
    }

    /// Resolve target for an agent spec
    pub fn resolve_for_agent(
        &self,
        agent_name: &str,
        agent_description: &str,
        explicit_target: Option<&str>,
        fallback: &Target,
    ) -> Target {
        // Explicit target takes priority
        if let Some(target_str) = explicit_target {
            if let Some(target) = Target::parse(target_str) {
                return target;
            }
        }

        // Infer category and route
        let category = RouteCategory::from_agent_name(agent_name, agent_description);
        self.resolve(category, fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_inference() {
        assert_eq!(
            RouteCategory::from_agent_name("planner", "Plan the architecture"),
            RouteCategory::Planning
        );
        assert_eq!(
            RouteCategory::from_agent_name("patch", "Apply code edits"),
            RouteCategory::Coding
        );
        assert_eq!(
            RouteCategory::from_agent_name("scout", "Find files"),
            RouteCategory::Exploration
        );
        assert_eq!(
            RouteCategory::from_agent_name("test-runner", "Run tests"),
            RouteCategory::Testing
        );
        assert_eq!(
            RouteCategory::from_agent_name("docs", "Write documentation"),
            RouteCategory::Documentation
        );
        assert_eq!(
            RouteCategory::from_agent_name("unknown", "Some agent"),
            RouteCategory::Default
        );
    }

    #[test]
    fn test_router_explicit_target_priority() {
        let router = ModelRouter::new(ModelRoutingConfig::default());
        let fallback = Target {
            model: "fallback".to_string(),
            backend: "test".to_string(),
        };

        let result =
            router.resolve_for_agent("scout", "Find files", Some("explicit@backend"), &fallback);
        assert_eq!(result.model, "explicit");
        assert_eq!(result.backend, "backend");
    }
}
