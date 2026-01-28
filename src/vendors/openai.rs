//! OpenAI usage and pricing integration.
//!
//! Provides pricing data for OpenAI models.
//! OpenAI's pricing is published on their website and embedded in API responses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cost::ModelPricing;

/// Cache expiry duration (1 week)
const CACHE_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

/// Cached pricing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIPricingCache {
    /// Unix timestamp when cache was last updated
    pub updated_at: u64,
    /// Model pricing: model_id -> (input_price, output_price) per 1M tokens
    pub models: HashMap<String, ModelPricing>,
    /// Source of the pricing data
    pub source: String,
}

impl OpenAIPricingCache {
    /// Check if cache is still valid
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.updated_at) < CACHE_MAX_AGE_SECS
    }
}

/// Get the cache file path (~/.brainpro/openai_pricing.json)
fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".brainpro").join("openai_pricing.json"))
}

/// Load cached pricing from disk
pub fn load_cache() -> Option<OpenAIPricingCache> {
    let path = cache_path()?;
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save pricing cache to disk
fn save_cache(cache: &OpenAIPricingCache) -> anyhow::Result<()> {
    let path = cache_path().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(cache)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Get the latest known OpenAI pricing.
///
/// Pricing source: https://openai.com/api/pricing/
/// Last updated: January 2025
///
/// Returns a map of model ID to ModelPricing (per 1M tokens in USD).
pub fn get_latest_pricing() -> HashMap<String, ModelPricing> {
    let mut models = HashMap::new();

    // GPT-4o family
    models.insert("gpt-4o".to_string(), ModelPricing::new(2.50, 10.00));
    models.insert(
        "gpt-4o-2024-11-20".to_string(),
        ModelPricing::new(2.50, 10.00),
    );
    models.insert(
        "gpt-4o-2024-08-06".to_string(),
        ModelPricing::new(2.50, 10.00),
    );
    models.insert(
        "gpt-4o-2024-05-13".to_string(),
        ModelPricing::new(5.00, 15.00),
    ); // Legacy pricing

    // GPT-4o mini
    models.insert("gpt-4o-mini".to_string(), ModelPricing::new(0.15, 0.60));
    models.insert(
        "gpt-4o-mini-2024-07-18".to_string(),
        ModelPricing::new(0.15, 0.60),
    );

    // GPT-4 Turbo
    models.insert("gpt-4-turbo".to_string(), ModelPricing::new(10.00, 30.00));
    models.insert(
        "gpt-4-turbo-2024-04-09".to_string(),
        ModelPricing::new(10.00, 30.00),
    );
    models.insert(
        "gpt-4-turbo-preview".to_string(),
        ModelPricing::new(10.00, 30.00),
    );

    // GPT-4 (original)
    models.insert("gpt-4".to_string(), ModelPricing::new(30.00, 60.00));
    models.insert("gpt-4-0613".to_string(), ModelPricing::new(30.00, 60.00));
    models.insert("gpt-4-32k".to_string(), ModelPricing::new(60.00, 120.00));

    // GPT-3.5 Turbo
    models.insert("gpt-3.5-turbo".to_string(), ModelPricing::new(0.50, 1.50));
    models.insert(
        "gpt-3.5-turbo-0125".to_string(),
        ModelPricing::new(0.50, 1.50),
    );
    models.insert(
        "gpt-3.5-turbo-instruct".to_string(),
        ModelPricing::new(1.50, 2.00),
    );

    // O1 reasoning models
    models.insert("o1".to_string(), ModelPricing::new(15.00, 60.00));
    models.insert("o1-2024-12-17".to_string(), ModelPricing::new(15.00, 60.00));
    models.insert("o1-preview".to_string(), ModelPricing::new(15.00, 60.00));
    models.insert(
        "o1-preview-2024-09-12".to_string(),
        ModelPricing::new(15.00, 60.00),
    );

    // O1 mini
    models.insert("o1-mini".to_string(), ModelPricing::new(3.00, 12.00));
    models.insert(
        "o1-mini-2024-09-12".to_string(),
        ModelPricing::new(3.00, 12.00),
    );

    // O3 models (latest reasoning)
    models.insert("o3".to_string(), ModelPricing::new(2.00, 8.00));
    models.insert("o3-mini".to_string(), ModelPricing::new(1.10, 4.40));
    models.insert("o3-mini-high".to_string(), ModelPricing::new(1.10, 4.40));

    // Embedding models (for completeness)
    models.insert(
        "text-embedding-3-small".to_string(),
        ModelPricing::new(0.02, 0.00),
    );
    models.insert(
        "text-embedding-3-large".to_string(),
        ModelPricing::new(0.13, 0.00),
    );
    models.insert(
        "text-embedding-ada-002".to_string(),
        ModelPricing::new(0.10, 0.00),
    );

    models
}

/// Get OpenAI pricing, using cache if valid or returning latest known prices.
pub fn get_openai_pricing() -> HashMap<String, ModelPricing> {
    // Try to load from cache first
    if let Some(cache) = load_cache() {
        if cache.is_valid() {
            return cache.models;
        }
    }

    // Return latest known pricing and update cache
    let models = get_latest_pricing();

    let cache = OpenAIPricingCache {
        updated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        models: models.clone(),
        source: "static_table".to_string(),
    };

    let _ = save_cache(&cache);

    models
}

/// OpenAI usage data from API response
#[derive(Debug, Clone, Deserialize)]
pub struct OpenAIUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    /// Reasoning tokens (for o1/o3 models)
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
    /// Cached tokens (prompt cache)
    #[serde(default)]
    pub cached_tokens: Option<u64>,
}

impl OpenAIUsage {
    /// Calculate cost based on pricing
    pub fn calculate_cost(&self, pricing: &ModelPricing) -> f64 {
        // For reasoning models, reasoning tokens are charged at output rate
        let reasoning = self.reasoning_tokens.unwrap_or(0);

        // Cached tokens get 50% discount
        let cached = self.cached_tokens.unwrap_or(0);
        let regular_prompt = self.prompt_tokens.saturating_sub(cached);

        let prompt_cost = (regular_prompt as f64 / 1_000_000.0) * pricing.input;
        let cached_cost = (cached as f64 / 1_000_000.0) * pricing.input * 0.5;
        let completion_cost = (self.completion_tokens as f64 / 1_000_000.0) * pricing.output;
        let reasoning_cost = (reasoning as f64 / 1_000_000.0) * pricing.output;

        prompt_cost + cached_cost + completion_cost + reasoning_cost
    }

    pub fn total(&self) -> u64 {
        if self.total_tokens > 0 {
            self.total_tokens
        } else {
            self.prompt_tokens + self.completion_tokens
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_latest_pricing() {
        let pricing = get_latest_pricing();
        assert!(pricing.contains_key("gpt-4o"));
        assert!(pricing.contains_key("o1"));

        let gpt4o = pricing.get("gpt-4o").unwrap();
        assert_eq!(gpt4o.input, 2.50);
        assert_eq!(gpt4o.output, 10.00);
    }

    #[test]
    fn test_usage_cost_calculation() {
        let usage = OpenAIUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 500_000,
            total_tokens: 1_500_000,
            reasoning_tokens: None,
            cached_tokens: None,
        };

        let pricing = ModelPricing::new(2.50, 10.00); // gpt-4o pricing
        let cost = usage.calculate_cost(&pricing);

        // 1M prompt * $2.50/1M + 500k completion * $10/1M = $2.50 + $5.00 = $7.50
        assert!((cost - 7.50).abs() < 0.01);
    }

    #[test]
    fn test_usage_with_caching() {
        let usage = OpenAIUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 100_000,
            total_tokens: 1_100_000,
            reasoning_tokens: None,
            cached_tokens: Some(500_000), // Half cached
        };

        let pricing = ModelPricing::new(2.50, 10.00);
        let cost = usage.calculate_cost(&pricing);

        // 500k regular * $2.50/1M + 500k cached * $1.25/1M + 100k completion * $10/1M
        // = $1.25 + $0.625 + $1.00 = $2.875
        assert!((cost - 2.875).abs() < 0.01);
    }

    #[test]
    fn test_cache_validity() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let fresh = OpenAIPricingCache {
            updated_at: now,
            models: HashMap::new(),
            source: "test".to_string(),
        };
        assert!(fresh.is_valid());
    }
}
