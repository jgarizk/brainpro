//! SKILL.md file parser.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::path::Path;

/// Validation constants
const MAX_NAME_LEN: usize = 64;
const MAX_DESCRIPTION_LEN: usize = 1024;

/// Parsed SKILL.md frontmatter
#[derive(Debug, Clone, Deserialize)]
pub struct SkillFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Option<AllowedTools>,
}

/// Allowed tools can be CSV string or YAML list
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum AllowedTools {
    Csv(String),
    List(Vec<String>),
}

impl AllowedTools {
    /// Convert to a list of tool names
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            AllowedTools::Csv(s) => s.split(',').map(|t| t.trim().to_string()).collect(),
            AllowedTools::List(v) => v.clone(),
        }
    }
}

/// Complete skill pack with frontmatter and body
#[derive(Debug, Clone)]
pub struct SkillPack {
    pub name: String,
    pub description: String,
    pub allowed_tools: Option<Vec<String>>,
    pub instructions: String,
    #[allow(dead_code)]
    pub root_path: std::path::PathBuf,
}

/// Parse only the frontmatter from a SKILL.md file (for indexing)
pub fn parse_frontmatter(content: &str) -> Result<SkillFrontmatter> {
    // Find YAML frontmatter between --- markers
    if !content.starts_with("---") {
        return Err(anyhow!("SKILL.md must start with YAML frontmatter (---)"));
    }

    let rest = &content[3..];
    let end = rest
        .find("\n---")
        .ok_or_else(|| anyhow!("Missing closing --- for frontmatter"))?;

    let yaml = &rest[..end];
    let frontmatter: SkillFrontmatter = serde_yaml::from_str(yaml)?;

    // Validate
    validate_name(&frontmatter.name)?;
    validate_description(&frontmatter.description)?;

    Ok(frontmatter)
}

/// Parse complete SKILL.md file (for activation)
pub fn parse_skill_md(path: &Path) -> Result<SkillPack> {
    let content = std::fs::read_to_string(path)?;
    let frontmatter = parse_frontmatter(&content)?;

    // Extract body after frontmatter
    // Find the second --- and take everything after
    let rest = &content[3..];
    let fm_end = rest
        .find("\n---")
        .ok_or_else(|| anyhow!("Missing closing --- for frontmatter"))?;
    let body_start = 3 + fm_end + 4; // skip "---" + "\n---"
    let instructions = if body_start < content.len() {
        content[body_start..].trim().to_string()
    } else {
        String::new()
    };

    let root_path = path.parent().unwrap_or(Path::new(".")).to_path_buf();

    Ok(SkillPack {
        name: frontmatter.name,
        description: frontmatter.description,
        allowed_tools: frontmatter.allowed_tools.map(|at| at.to_vec()),
        instructions,
        root_path,
    })
}

fn validate_name(name: &str) -> Result<()> {
    if name.len() > MAX_NAME_LEN {
        return Err(anyhow!("Skill name exceeds {} chars", MAX_NAME_LEN));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(anyhow!(
            "Skill name must be lowercase letters, numbers, hyphens only"
        ));
    }
    Ok(())
}

fn validate_description(desc: &str) -> Result<()> {
    if desc.len() > MAX_DESCRIPTION_LEN {
        return Err(anyhow!("Description exceeds {} chars", MAX_DESCRIPTION_LEN));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
name: safe-file-reader
description: Read files without making changes
allowed-tools: Read, Grep, Glob
---

Only inspect files; do not modify.
"#;
        let fm = parse_frontmatter(content).unwrap();
        assert_eq!(fm.name, "safe-file-reader");
        assert_eq!(fm.description, "Read files without making changes");
        let tools = fm.allowed_tools.unwrap().to_vec();
        assert_eq!(tools, vec!["Read", "Grep", "Glob"]);
    }

    #[test]
    fn test_parse_frontmatter_yaml_list() {
        let content = r#"---
name: test-skill
description: A test skill
allowed-tools:
  - Read
  - Write
---

Instructions here.
"#;
        let fm = parse_frontmatter(content).unwrap();
        let tools = fm.allowed_tools.unwrap().to_vec();
        assert_eq!(tools, vec!["Read", "Write"]);
    }

    #[test]
    fn test_invalid_name() {
        let content = r#"---
name: Invalid_Name
description: Bad name
---
"#;
        let result = parse_frontmatter(content);
        assert!(result.is_err());
    }
}
