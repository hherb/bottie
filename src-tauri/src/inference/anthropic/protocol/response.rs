//! Bounded reconstruction of streamed Anthropic assistant content blocks.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{
    AnthropicAssistantBlock, AnthropicToolCall, AnthropicToolRound, ContentDelta, DecodedEvent,
    StartBlock,
};
use crate::inference::{ProviderError, Usage};

const MAX_PROVIDER_CALL_ID_CHARACTERS: usize = 512;
const MAX_TOOL_NAME_CHARACTERS: usize = 128;
const MAX_STREAMED_TOOL_CALLS: usize = 64;
const MAX_STREAMED_ARGUMENT_BYTES: usize = 64 * 1_024;

/// Reconstructs ordered Messages content blocks and tool arguments across streamed fragments.
#[derive(Default)]
pub(in crate::inference::anthropic) struct AnthropicResponseAccumulator {
    active: BTreeMap<usize, PartialBlock>,
    complete: BTreeMap<usize, AnthropicAssistantBlock>,
    usage: Usage,
    saw_usage: bool,
    stop_reason: Option<String>,
    completed: bool,
}

enum PartialBlock {
    Text(String),
    Thinking {
        thinking: String,
        signature: String,
    },
    RedactedThinking(String),
    ToolUse {
        call_id: String,
        tool_name: String,
        initial_input: Value,
        input_json: String,
    },
    Ignored,
}

impl AnthropicResponseAccumulator {
    /// Applies one decoded event while enforcing block identity and bounded call accumulation.
    pub(in crate::inference::anthropic) fn apply(
        &mut self,
        event: DecodedEvent,
    ) -> Result<(), ProviderError> {
        match event {
            DecodedEvent::BlockStart { index, block } => self.start_block(index, block)?,
            DecodedEvent::BlockDelta { index, delta } => self.apply_delta(index, delta)?,
            DecodedEvent::BlockStop { index } => self.stop_block(index)?,
            DecodedEvent::Usage(usage) => self.merge_usage(usage),
            DecodedEvent::MessageDelta { stop_reason, usage } => {
                if stop_reason.is_some() {
                    self.stop_reason = stop_reason;
                }
                self.merge_usage(usage);
            }
            DecodedEvent::Done => self.completed = true,
            DecodedEvent::Ignored => {}
        }
        Ok(())
    }

    /// Returns the latest request-local usage checkpoint after a usage event.
    pub(in crate::inference::anthropic) fn usage(&self) -> Option<Usage> {
        self.saw_usage.then(|| self.usage.clone())
    }

    /// Reports whether the required `message_stop` event has arrived.
    pub(in crate::inference::anthropic) fn is_complete(&self) -> bool {
        self.completed
    }

    /// Finalizes one complete round and validates provider call identities and argument objects.
    pub(in crate::inference::anthropic) fn finish(
        self,
    ) -> Result<AnthropicToolRound, ProviderError> {
        if !self.completed || !self.active.is_empty() {
            return Err(malformed_tool_call(
                "stream ended with incomplete content blocks",
            ));
        }
        let assistant_blocks = self.complete.into_values().collect::<Vec<_>>();
        let tool_calls = assistant_blocks
            .iter()
            .filter_map(|block| match block {
                AnthropicAssistantBlock::ToolUse { id, name, input } => Some(AnthropicToolCall {
                    call_id: id.clone(),
                    tool_name: name.clone(),
                    arguments: input.clone(),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        if tool_calls.len() > MAX_STREAMED_TOOL_CALLS {
            return Err(malformed_tool_call("too many streamed tool calls"));
        }
        if !tool_calls.is_empty() && self.stop_reason.as_deref() != Some("tool_use") {
            return Err(malformed_tool_call(
                "tool calls did not end with the tool_use stop reason",
            ));
        }
        if !tool_calls.is_empty()
            && assistant_blocks.iter().any(|block| {
                matches!(block, AnthropicAssistantBlock::Thinking { signature, .. } if signature.is_empty())
            })
        {
            return Err(malformed_tool_call(
                "thinking block omitted its required signature",
            ));
        }
        Ok(AnthropicToolRound {
            assistant_blocks,
            tool_calls,
            usage: self.saw_usage.then_some(self.usage),
        })
    }

    fn start_block(&mut self, index: usize, block: StartBlock) -> Result<(), ProviderError> {
        if self.active.contains_key(&index) || self.complete.contains_key(&index) {
            return Err(malformed_tool_call("duplicate content block index"));
        }
        let block = match block {
            StartBlock::Text { text } => PartialBlock::Text(text),
            StartBlock::Thinking {
                thinking,
                signature,
            } => PartialBlock::Thinking {
                thinking,
                signature,
            },
            StartBlock::RedactedThinking { data } => PartialBlock::RedactedThinking(data),
            StartBlock::ToolUse { id, name, input } => {
                if self.tool_call_count() >= MAX_STREAMED_TOOL_CALLS {
                    return Err(malformed_tool_call("too many streamed tool calls"));
                }
                validate_identity(
                    &id,
                    MAX_PROVIDER_CALL_ID_CHARACTERS,
                    "invalid provider call identity",
                )?;
                validate_identity(&name, MAX_TOOL_NAME_CHARACTERS, "invalid tool name")?;
                if !input.is_object() {
                    return Err(malformed_tool_call("initial tool input was not an object"));
                }
                PartialBlock::ToolUse {
                    call_id: id,
                    tool_name: name,
                    initial_input: input,
                    input_json: String::new(),
                }
            }
            StartBlock::Unknown => PartialBlock::Ignored,
        };
        self.active.insert(index, block);
        Ok(())
    }

    fn tool_call_count(&self) -> usize {
        self.active
            .values()
            .filter(|block| matches!(block, PartialBlock::ToolUse { .. }))
            .count()
            + self
                .complete
                .values()
                .filter(|block| matches!(block, AnthropicAssistantBlock::ToolUse { .. }))
                .count()
    }

    fn apply_delta(&mut self, index: usize, delta: ContentDelta) -> Result<(), ProviderError> {
        let block = self
            .active
            .get_mut(&index)
            .ok_or_else(|| malformed_tool_call("delta referenced an unknown content block"))?;
        match (block, delta) {
            (PartialBlock::Text(text), ContentDelta::TextDelta { text: delta }) => {
                text.push_str(&delta)
            }
            (
                PartialBlock::Thinking { thinking, .. },
                ContentDelta::ThinkingDelta { thinking: delta },
            ) => thinking.push_str(&delta),
            (
                PartialBlock::Thinking { signature, .. },
                ContentDelta::SignatureDelta { signature: delta },
            ) => signature.push_str(&delta),
            (
                PartialBlock::ToolUse { input_json, .. },
                ContentDelta::InputJsonDelta { partial_json },
            ) => {
                if input_json.len().saturating_add(partial_json.len()) > MAX_STREAMED_ARGUMENT_BYTES
                {
                    return Err(malformed_tool_call(
                        "streamed tool arguments exceeded their limit",
                    ));
                }
                input_json.push_str(&partial_json);
            }
            (PartialBlock::Ignored, _) | (_, ContentDelta::Unknown) => {}
            _ => {
                return Err(malformed_tool_call(
                    "content delta did not match its block type",
                ));
            }
        }
        Ok(())
    }

    fn stop_block(&mut self, index: usize) -> Result<(), ProviderError> {
        let block = self
            .active
            .remove(&index)
            .ok_or_else(|| malformed_tool_call("stop referenced an unknown content block"))?;
        let block = match block {
            PartialBlock::Text(text) => AnthropicAssistantBlock::Text { text },
            PartialBlock::Thinking {
                thinking,
                signature,
            } => AnthropicAssistantBlock::Thinking {
                thinking,
                signature,
            },
            PartialBlock::RedactedThinking(data) => {
                AnthropicAssistantBlock::RedactedThinking { data }
            }
            PartialBlock::ToolUse {
                call_id,
                tool_name,
                initial_input,
                input_json,
            } => {
                let input = if input_json.is_empty() {
                    initial_input
                } else {
                    let input: Value = serde_json::from_str(&input_json).map_err(|_| {
                        malformed_tool_call("tool arguments were not complete JSON")
                    })?;
                    if !input.is_object() {
                        return Err(malformed_tool_call("tool arguments were not a JSON object"));
                    }
                    input
                };
                AnthropicAssistantBlock::ToolUse {
                    id: call_id,
                    name: tool_name,
                    input,
                }
            }
            PartialBlock::Ignored => return Ok(()),
        };
        self.complete.insert(index, block);
        Ok(())
    }

    fn merge_usage(&mut self, usage: Usage) {
        if usage.input_tokens.is_some() {
            self.usage.input_tokens = usage.input_tokens;
        }
        if usage.output_tokens.is_some() {
            self.usage.output_tokens = usage.output_tokens;
        }
        if usage.cost_usd.is_some() {
            self.usage.cost_usd = usage.cost_usd;
        }
        self.saw_usage = true;
    }
}

fn validate_identity(
    value: &str,
    maximum: usize,
    diagnostic: &'static str,
) -> Result<(), ProviderError> {
    if value.trim().is_empty() || value.chars().count() > maximum {
        Err(malformed_tool_call(diagnostic))
    } else {
        Ok(())
    }
}

fn malformed_tool_call(diagnostic: &'static str) -> ProviderError {
    ProviderError::malformed(
        "The Anthropic-compatible provider sent an invalid native tool call.",
        Some(diagnostic.into()),
    )
}
