//! Validated, durable budgets for provider output and native tool execution.

use serde::{Deserialize, Serialize};

use super::ProviderError;

/// Default completion allowance used by every provider route.
pub(crate) const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 8_192;
/// Largest user-selectable number of tool rounds per answer.
const MAX_CONFIGURED_TOOL_ROUNDS: usize = 64;
/// Largest user-selectable number of total tool calls per answer.
const MAX_CONFIGURED_TOOL_CALLS: usize = 128;
/// Largest user-selectable output allowance; providers may enforce a lower ceiling.
const MAX_CONFIGURED_OUTPUT_TOKENS: u32 = 65_536;

/// Secret-free generation preferences saved with provider Settings.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct GenerationLimits {
    /// Maximum provider-to-tool rounds in one answer.
    pub max_tool_rounds: usize,
    /// Maximum total calls across those rounds.
    pub max_tool_calls: usize,
    /// Maximum generated tokens in each provider request.
    pub max_output_tokens: u32,
}

impl Default for GenerationLimits {
    fn default() -> Self {
        Self {
            max_tool_rounds: crate::tool_loop::MAX_TOOL_LOOP_ROUNDS,
            max_tool_calls: crate::tool_loop::MAX_TOOL_LOOP_CALLS,
            max_output_tokens: DEFAULT_MAX_OUTPUT_TOKENS,
        }
    }
}

impl GenerationLimits {
    /// Rejects invalid values at the native persistence boundary, regardless of UI constraints.
    pub(crate) fn validated(self) -> Result<Self, ProviderError> {
        if !(1..=MAX_CONFIGURED_TOOL_ROUNDS).contains(&self.max_tool_rounds)
            || !(1..=MAX_CONFIGURED_TOOL_CALLS).contains(&self.max_tool_calls)
            || !(1..=MAX_CONFIGURED_OUTPUT_TOKENS).contains(&self.max_output_tokens)
        {
            return Err(ProviderError::invalid_request(
                "Generation limits require 1–64 tool rounds, 1–128 total tool calls, and 1–65536 output tokens.",
            ));
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::{ProviderSettings, load_provider_settings, save_provider_settings};

    #[test]
    fn saved_generation_limits_survive_restart_and_legacy_settings_gain_defaults() {
        let path =
            std::env::temp_dir().join(format!("bottie-limits-{}.json", uuid::Uuid::new_v4()));
        let limits = GenerationLimits {
            max_tool_rounds: 16,
            max_tool_calls: 40,
            max_output_tokens: 12_288,
        };
        let settings = ProviderSettings {
            generation_limits: limits,
            ..Default::default()
        };
        save_provider_settings(&path, &settings).unwrap();
        assert_eq!(
            load_provider_settings(&path).unwrap().generation_limits,
            limits
        );
        let legacy: ProviderSettings = serde_json::from_str(
            r#"{"omlxBaseUrl":"http://127.0.0.1:8000/","ollamaBaseUrl":"http://127.0.0.1:11434/"}"#,
        )
        .unwrap();
        assert_eq!(legacy.generation_limits, GenerationLimits::default());
        assert!(
            ProviderSettings {
                generation_limits: GenerationLimits {
                    max_tool_rounds: 0,
                    ..limits
                },
                ..Default::default()
            }
            .normalized()
            .is_err()
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn defaults_allow_twelve_rounds_and_eight_k_output_tokens() {
        let limits = GenerationLimits::default();
        assert_eq!(limits.max_tool_rounds, 12);
        assert_eq!(limits.max_tool_calls, 24);
        assert_eq!(limits.max_output_tokens, 8_192);
        assert_eq!(
            serde_json::from_str::<GenerationLimits>("{}").unwrap(),
            limits
        );
    }

    #[test]
    fn rejects_invalid_or_foreign_generation_limits() {
        for field in ["maxToolRounds", "maxToolCalls", "maxOutputTokens"] {
            for value in [0, 100_000] {
                let limits: GenerationLimits =
                    serde_json::from_value(serde_json::json!({field: value})).unwrap();
                assert!(limits.validated().is_err());
            }
        }
        assert!(serde_json::from_str::<GenerationLimits>(r#"{"foreign":1}"#).is_err());
        assert!(serde_json::from_str::<GenerationLimits>(r#"{"maxToolRounds":1.5}"#).is_err());
    }
}
