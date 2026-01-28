//! Anthropic usage API integration.
//!
//! Fetches real-time pricing and usage data from Anthropic's API.
//! Note: Anthropic's pricing is typically accessed via their website or
//! embedded in API responses rather than a dedicated pricing endpoint.
//!
//! This module provides:
//! - Static pricing table (updated periodically)
//! - Usage tracking from API responses
//! - Cache management

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cost::ModelPricing;

/// Cache expiry duration (1 week)
const CACHE_MAX_AGE_SECS: u64 = 7 * 24 * 60 * 60;

/// Cached pricing data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicPricingCache {
    /// Unix timestamp when cache was last updated
    pub updated_at: u64,
    /// Model pricing: model_id -> (input_price, output_price) per 1M tokens
    pub models: HashMap<String, ModelPricing>,
    /// Source of the pricing data
    pub source: String,
}

impl AnthropicPricingCache {
    /// Check if cache is still valid
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now.saturating_sub(self.updated_at) < CACHE_MAX_AGE_SECS
    }
}

/// Get the cache file path (~/.brainpro/anthropic_pricing.json)
fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".brainpro").join("anthropic_pricing.json"))
}

/// Load cached pricing from disk
pub fn load_cache() -> Option<AnthropicPricingCache> {
    let path = cache_path()?;
    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save pricing cache to disk
fn save_cache(cache: &AnthropicPricingCache) -> anyhow::Result<()> {
    let path = cache_path().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(cache)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Get the latest known Anthropic pricing.
///
/// Pricing source: https://www.anthropic.com/pricing
/// Last updated: January 2025
///
/// Returns a map of model ID to ModelPricing (per 1M tokens in USD).
pub fn get_latest_pricing() -> HashMap<String, ModelPricing> {
    let mut models = HashMap::new();

    // Claude 3.5 Sonnet (most common)
    models.insert(
        "claude-3-5-sonnet-latest".to_string(),
        ModelPricing::new(3.00, 15.00),
    );
    models.insert(
        "claude-3-5-sonnet-20241022".to_string(),
        ModelPricing::new(3.00, 15.00),
    );
    models.insert(
        "claude-3-5-sonnet-20240620".to_string(),
        ModelPricing::new(3.00, 15.00),
    );

    // Claude 3.5 Haiku
    models.insert(
        "claude-3-5-haiku-latest".to_string(),
        ModelPricing::new(0.80, 4.00),
    );
    models.insert(
        "claude-3-5-haiku-20241022".to_string(),
        ModelPricing::new(0.80, 4.00),
    );

    // Claude 3 Opus
    models.insert(
        "claude-3-opus-latest".to_string(),
        ModelPricing::new(15.00, 75.00),
    );
    models.insert(
        "claude-3-opus-20240229".to_string(),
        ModelPricing::new(15.00, 75.00),
    );

    // Claude 3 Sonnet
    models.insert(
        "claude-3-sonnet-20240229".to_string(),
        ModelPricing::new(3.00, 15.00),
    );

    // Claude 3 Haiku
    models.insert(
        "claude-3-haiku-20240307".to_string(),
        ModelPricing::new(0.25, 1.25),
    );

    // Claude 4.x models (next generation)
    models.insert(
        "claude-opus-4-5-20251101".to_string(),
        ModelPricing::new(5.00, 25.00),
    );
    models.insert(
        "claude-opus-4.5".to_string(),
        ModelPricing::new(5.00, 25.00),
    );
    models.insert("claude-opus-4".to_string(), ModelPricing::new(15.00, 75.00));
    models.insert(
        "claude-sonnet-4.5".to_string(),
        ModelPricing::new(3.00, 15.00),
    );
    models.insert("claude-sonnet-4".to_string(), ModelPricing::new(3.00, 15.00));
    models.insert(
        "claude-haiku-4.5".to_string(),
        ModelPricing::new(1.00, 5.00),
    );

    models
}

/// Get Anthropic pricing, using cache if valid or returning latest known prices.
pub fn get_anthropic_pricing() -> HashMap<String, ModelPricing> {
    // Try to load from cache first
    if let Some(cache) = load_cache() {
        if cache.is_valid() {
            return cache.models;
        }
    }

    // Return latest known pricing and update cache
    let models = get_latest_pricing();

    let cache = AnthropicPricingCache {
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

/// Record usage from an API response.
///
/// Anthropic API responses include usage data in the response body.
/// This function extracts and records it.
#[derive(Debug, Clone, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_creation_input_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: u64,
}

impl AnthropicUsage {
    /// Calculate cost based on pricing
    pub fn calculate_cost(&self, pricing: &ModelPricing) -> f64 {
        // Regular tokens
        let input_cost = (self.input_tokens as f64 / 1_000_000.0) * pricing.input;
        let output_cost = (self.output_tokens as f64 / 1_000_000.0) * pricing.output;

        // Cache tokens (90% discount for cache creation, cache reads are free)
        let cache_creation_cost =
            (self.cache_creation_input_tokens as f64 / 1_000_000.0) * pricing.input * 1.25;

        input_cost + output_cost + cache_creation_cost
    }

    pub fn total_tokens(&self) -> u64 {
        self.input_tokens
            + self.output_tokens
            + self.cache_creation_input_tokens
            + self.cache_read_input_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_latest_pricing() {
        let pricing = get_latest_pricing();
        assert!(pricing.contains_key("claude-3-5-sonnet-latest"));
        assert!(pricing.contains_key("claude-3-opus-latest"));

        let sonnet = pricing.get("claude-3-5-sonnet-latest").unwrap();
        assert_eq!(sonnet.input, 3.00);
        assert_eq!(sonnet.output, 15.00);
    }

    #[test]
    fn test_usage_cost_calculation() {
        let usage = AnthropicUsage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };

        let pricing = ModelPricing::new(3.00, 15.00);
        let cost = usage.calculate_cost(&pricing);

        // 1M input * $3/1M + 500k output * $15/1M = $3 + $7.50 = $10.50
        assert!((cost - 10.50).abs() < 0.01);
    }

    #[test]
    fn test_cache_validity() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let fresh = AnthropicPricingCache {
            updated_at: now,
            models: HashMap::new(),
            source: "test".to_string(),
        };
        assert!(fresh.is_valid());

        let old = AnthropicPricingCache {
            updated_at: now - CACHE_MAX_AGE_SECS - 1,
            models: HashMap::new(),
            source: "test".to_string(),
        };
        assert!(!old.is_valid());
    }
}
