//! Skill Packs: portable, repo-checkable skill system.
//!
//! Skill packs are SKILL.md files with YAML frontmatter that provide
//! reusable instructions to the agent.

pub mod activation;
pub mod index;
pub mod parser;

pub use activation::ActiveSkills;
pub use index::SkillIndex;
pub use parser::SkillPack;
