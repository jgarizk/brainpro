//! MrCode personality - focused coding assistant.
//!
//! MrCode is a direct, terse coding assistant designed for:
//! - On-demand local agent via Unix socket
//! - Minimal toolset: Read, Write, Edit, Glob, Grep, Bash
//! - Simple system prompt loaded from config/personalities/mrcode/

mod loop_impl;

use crate::agent::TurnResult;
use crate::cli::Context;
use crate::config::PermissionMode;
use crate::personality::loader::{self, PersonalityConfig};
use crate::personality::{Personality, PromptContext};
use anyhow::Result;
use serde_json::Value;

/// MrCode personality - focused coding assistant
pub struct MrCode {
    /// Loaded configuration from files
    config: PersonalityConfig,
    /// Cached tools as static refs
    tools: Vec<&'static str>,
}

impl MrCode {
    /// Create a new MrCode personality
    pub fn new() -> Self {
        let config = loader::load_personality("mrcode")
            .expect("Failed to load mrcode personality config");
        let tools = config.tools_as_static();
        Self { config, tools }
    }
}

impl Default for MrCode {
    fn default() -> Self {
        Self::new()
    }
}

impl Personality for MrCode {
    fn name(&self) -> &str {
        "MrCode"
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
