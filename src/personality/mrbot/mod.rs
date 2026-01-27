//! MrBot personality - conversational bot with personality.
//!
//! MrBot is designed for:
//! - Gateway/daemon architecture (yield/resume)
//! - Full toolset including messaging/voice (future)
//! - Personality loaded from config/personalities/mrbot/
//! - Modular prompt builder (clawdbot-style)

mod loop_impl;

use crate::agent::TurnResult;
use crate::cli::Context;
use crate::config::PermissionMode;
use crate::personality::loader::{self, PersonalityConfig};
use crate::personality::{Personality, PromptContext};
use anyhow::Result;
use serde_json::Value;

/// MrBot personality - conversational bot with SOUL
pub struct MrBot {
    /// Loaded configuration from files
    config: PersonalityConfig,
    /// Cached tools as static refs
    tools: Vec<&'static str>,
}

impl MrBot {
    /// Create a new MrBot personality
    pub fn new() -> Self {
        let config = loader::load_personality("mrbot")
            .expect("Failed to load mrbot personality config");
        let tools = config.tools_as_static();
        Self { config, tools }
    }
}

impl Default for MrBot {
    fn default() -> Self {
        Self::new()
    }
}

impl Personality for MrBot {
    fn name(&self) -> &str {
        "MrBot"
    }

    fn config(&self) -> &PersonalityConfig {
        &self.config
    }

    fn build_system_prompt(&self, ctx: &PromptContext) -> String {
        loader::build_system_prompt(&self.config, ctx)
    }

    fn run_turn(
        &self,
        ctx: &Context,
        user_input: &str,
        messages: &mut Vec<Value>,
    ) -> Result<TurnResult> {
        loop_impl::run_turn(&self.config, ctx, user_input, messages)
    }

    fn available_tools(&self) -> &[&str] {
        &self.tools
    }

    fn permission_mode(&self) -> PermissionMode {
        self.config.permission_mode.clone()
    }
}
