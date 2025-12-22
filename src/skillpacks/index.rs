//! Skill pack discovery and indexing.

use super::parser::parse_frontmatter;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Source of a skill pack
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillSource {
    Project, // .yo/skills/
    User,    // ~/.yo/skills/
}

/// Minimal metadata for a skill (for progressive disclosure)
#[derive(Debug, Clone)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub allowed_tools: Option<Vec<String>>,
    pub path: PathBuf,
    pub source: SkillSource,
}

/// Index of all discovered skills
#[derive(Debug, Default)]
pub struct SkillIndex {
    skills: HashMap<String, SkillMetadata>,
    parse_errors: Vec<(PathBuf, String)>,
}

impl SkillIndex {
    /// Build index from search paths
    pub fn build(project_root: &Path) -> Self {
        let mut index = SkillIndex::default();

        // User skills (lower priority - loaded first, overwritten by project)
        if let Some(home) = dirs::home_dir() {
            let user_skills = home.join(".yo").join("skills");
            index.scan_dir(&user_skills, SkillSource::User);
        }

        // Project skills (higher priority)
        let project_skills = project_root.join(".yo").join("skills");
        index.scan_dir(&project_skills, SkillSource::Project);

        index
    }

    fn scan_dir(&mut self, dir: &Path, source: SkillSource) {
        if !dir.exists() {
            return;
        }

        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_md = path.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            match self.index_skill(&skill_md, source) {
                Ok(meta) => {
                    self.skills.insert(meta.name.clone(), meta);
                }
                Err(e) => {
                    self.parse_errors.push((skill_md, e.to_string()));
                }
            }
        }
    }

    fn index_skill(&self, path: &Path, source: SkillSource) -> Result<SkillMetadata> {
        let content = std::fs::read_to_string(path)?;
        let frontmatter = parse_frontmatter(&content)?;

        Ok(SkillMetadata {
            name: frontmatter.name.clone(),
            description: frontmatter.description,
            allowed_tools: frontmatter.allowed_tools.map(|at| at.to_vec()),
            path: path.to_path_buf(),
            source,
        })
    }

    /// Get all skill metadata
    pub fn all(&self) -> impl Iterator<Item = &SkillMetadata> {
        self.skills.values()
    }

    /// Get skill by name
    pub fn get(&self, name: &str) -> Option<&SkillMetadata> {
        self.skills.get(name)
    }

    /// Get parse errors
    pub fn errors(&self) -> &[(PathBuf, String)] {
        &self.parse_errors
    }

    /// Count of indexed skills
    pub fn count(&self) -> usize {
        self.skills.len()
    }

    /// Format skill list for system prompt injection
    pub fn format_for_prompt(&self, max_entries: usize) -> String {
        if self.skills.is_empty() {
            return String::new();
        }

        let mut lines = vec!["Available skill packs:".to_string()];

        for (count, meta) in self.skills.values().enumerate() {
            if count >= max_entries {
                let remaining = self.skills.len() - max_entries;
                lines.push(format!("  (+{} more; use /skillpacks to view)", remaining));
                break;
            }
            lines.push(format!("- {}: {}", meta.name, meta.description));
        }

        lines.join("\n")
    }
}
