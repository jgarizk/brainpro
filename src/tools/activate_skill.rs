//! ActivateSkill tool for model-invoked skill activation.

use serde_json::{json, Value};

/// Get the ActivateSkill tool schema
pub fn schema() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "ActivateSkill",
            "description": "Activate a skill pack to gain specialized instructions and optionally restrict available tools. Use when the task matches a skill's description. View available skills in the 'Available skill packs' section of the system prompt.",
            "parameters": {
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the skill pack to activate"
                    },
                    "reason": {
                        "type": "string",
                        "description": "Brief reason for activating this skill (optional)"
                    }
                },
                "required": ["name"]
            }
        }
    })
}
