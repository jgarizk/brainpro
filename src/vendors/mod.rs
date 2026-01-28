//! Vendor-specific integrations.
//!
//! Contains modules for vendor-specific API integrations,
//! pricing lookups, and other vendor-dependent functionality.

pub mod anthropic;
pub mod openai;
pub mod venice;

use crate::cost::PricingTable;

/// Load pricing from all vendors and merge into a unified pricing table.
///
/// Priority order (later overrides earlier):
/// 1. Default static pricing in PricingTable
/// 2. OpenAI pricing (static + cached)
/// 3. Anthropic pricing (static + cached)
/// 4. Venice pricing (API + cached)
///
/// Venice has highest priority because it fetches live from API.
pub fn load_all_pricing() -> PricingTable {
    let mut table = PricingTable::with_defaults();

    // Merge OpenAI pricing
    let openai = openai::get_openai_pricing();
    for (model, pricing) in openai {
        table.models.insert(model, pricing);
    }

    // Merge Anthropic pricing
    let anthropic = anthropic::get_anthropic_pricing();
    for (model, pricing) in anthropic {
        table.models.insert(model, pricing);
    }

    // Merge Venice pricing (live API, highest priority)
    if let Some(venice) = venice::get_venice_pricing() {
        for (model, pricing) in venice {
            table.models.insert(model, pricing);
        }
    }

    table
}

/// Pricing source information for diagnostics
#[derive(Debug, Clone)]
pub struct PricingSource {
    pub vendor: String,
    pub source_type: PricingSourceType,
    pub last_updated: Option<u64>,
    pub model_count: usize,
}

#[derive(Debug, Clone)]
pub enum PricingSourceType {
    /// Static table compiled into binary
    Static,
    /// Cached from previous API fetch
    Cached,
    /// Freshly fetched from API
    LiveApi,
}

impl std::fmt::Display for PricingSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PricingSourceType::Static => write!(f, "static"),
            PricingSourceType::Cached => write!(f, "cached"),
            PricingSourceType::LiveApi => write!(f, "live"),
        }
    }
}

/// Get diagnostic information about pricing sources
pub fn get_pricing_diagnostics() -> Vec<PricingSource> {
    let mut sources = Vec::new();

    // OpenAI
    let openai_models = openai::get_openai_pricing();
    let openai_cache = openai::load_cache();
    sources.push(PricingSource {
        vendor: "openai".to_string(),
        source_type: if openai_cache.as_ref().is_some_and(|c| c.is_valid()) {
            PricingSourceType::Cached
        } else {
            PricingSourceType::Static
        },
        last_updated: openai_cache.map(|c| c.updated_at),
        model_count: openai_models.len(),
    });

    // Anthropic
    let anthropic_models = anthropic::get_anthropic_pricing();
    let anthropic_cache = anthropic::load_cache();
    sources.push(PricingSource {
        vendor: "anthropic".to_string(),
        source_type: if anthropic_cache.as_ref().is_some_and(|c| c.is_valid()) {
            PricingSourceType::Cached
        } else {
            PricingSourceType::Static
        },
        last_updated: anthropic_cache.map(|c| c.updated_at),
        model_count: anthropic_models.len(),
    });

    // Venice
    let venice_cache = venice::load_cache();
    if let Some(ref cache) = venice_cache {
        sources.push(PricingSource {
            vendor: "venice".to_string(),
            source_type: if cache.is_valid() {
                PricingSourceType::Cached
            } else {
                PricingSourceType::Static
            },
            last_updated: Some(cache.fetched_at),
            model_count: cache.models.len(),
        });
    }

    sources
}

