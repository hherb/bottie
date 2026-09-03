//! Provider-independent native tool definitions and strict raw-argument validation.

mod current_time;
mod localmail;
mod python;
mod web_fetch;

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::storage::{
    MAX_MEMORY_CONVERSATION_ID_CHARACTERS, MAX_MEMORY_QUERY_CHARACTERS,
    MAX_OPEN_MEMORY_ID_CHARACTERS, MAX_OPEN_MEMORY_SURROUNDING_TURNS,
    MAX_SEARCH_ATTACHED_FILE_RESULTS, MAX_SEARCH_MEMORY_RESULTS, OPEN_MEMORY_TOOL_NAME,
    OpenMemoryArguments, SEARCH_ATTACHED_FILES_TOOL_NAME, SEARCH_MEMORY_TOOL_NAME,
    SearchAttachedFilesArguments, SearchMemoryArguments,
};
use crate::web_search::{
    MAX_WEB_SEARCH_DOMAIN_CHARS, MAX_WEB_SEARCH_FILTER_DOMAINS, MAX_WEB_SEARCH_QUERY_CHARS,
    MAX_WEB_SEARCH_TOOL_RESULTS, WEB_SEARCH_TOOL_NAME, WebSearchArguments,
};

pub(crate) use current_time::{
    CURRENT_TIME_TOOL_NAME, current_time_tool_definition, validate_current_time_tool_arguments,
};
pub(crate) use localmail::localmail_tool_definitions;
pub(crate) use localmail::{
    LocalmailToolArguments, OPEN_EMAIL_TOOL_NAME, READ_EMAIL_ATTACHMENT_TOOL_NAME,
    SEARCH_EMAIL_TOOL_NAME, validate_localmail_tool_arguments,
};
#[allow(
    unused_imports,
    reason = "the Python contract is reserved here while provider mapping remains deferred"
)]
pub(crate) use python::{
    MAX_PYTHON_PURPOSE_CHARACTERS, MAX_PYTHON_SOURCE_BYTES, PythonToolArguments,
    RUN_PYTHON_TOOL_NAME, python_tool_definition, validate_python_tool_arguments,
};
pub(crate) use web_fetch::{validate_web_fetch_tool_arguments, web_fetch_tool_definition};

/// Provider-neutral definition of one Rust-owned tool and its closed JSON input schema.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolDefinition {
    /// Stable name providers must return unchanged when requesting the tool.
    pub(crate) name: &'static str,
    /// Bounded model-facing guidance that does not expose implementation details.
    pub(crate) description: &'static str,
    /// Closed JSON Schema object applied before typed native dispatch.
    pub(crate) input_schema: Value,
}

/// Exact typed arguments produced only after a raw memory-tool call passes its schema contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MemoryToolArguments {
    /// Validated arguments for conversation-message retrieval.
    SearchMemory(SearchMemoryArguments),
    /// Validated exact provenance for surrounding-turn reconstruction.
    OpenMemory(OpenMemoryArguments),
    /// Validated arguments for retained-document retrieval.
    SearchAttachedFiles(SearchAttachedFilesArguments),
}

/// Stable category for a tool name or argument payload rejected at the native boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolContractErrorCode {
    /// The provider requested a name outside Bottie's advertised native tool set.
    UnsupportedTool,
    /// The provider supplied JSON that does not satisfy the selected tool's closed schema.
    InvalidArguments,
}

/// Redacted tool-contract error that never repeats provider-controlled argument content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ToolContractError {
    /// Stable machine-readable failure category.
    pub(crate) code: ToolContractErrorCode,
    /// Safe explanation suitable for later provider-loop diagnostics.
    pub(crate) message: &'static str,
}

/// Returns the complete ordered native memory-tool set without provider wire-format mapping.
pub(crate) fn memory_tool_definitions() -> [ToolDefinition; 3] {
    [
        ToolDefinition {
            name: SEARCH_MEMORY_TOOL_NAME,
            description: concat!(
                "Find relevant final conversation messages. Use open_memory with returned provenance when ",
                "surrounding turns are needed."
            ),
            input_schema: search_schema(MAX_SEARCH_MEMORY_RESULTS),
        },
        ToolDefinition {
            name: OPEN_MEMORY_TOOL_NAME,
            description: "Open a small window of final conversation turns around exact search_memory provenance.",
            input_schema: open_schema(),
        },
        ToolDefinition {
            name: SEARCH_ATTACHED_FILES_TOOL_NAME,
            description: concat!(
                "Find relevant excerpts in retained extracted documents associated with active or archived ",
                "conversations."
            ),
            input_schema: search_schema(MAX_SEARCH_ATTACHED_FILE_RESULTS),
        },
    ]
}

/// Returns exactly the native definitions enabled for one already-capable provider request.
pub(crate) fn enabled_native_tool_definitions(
    memory_enabled: bool,
    web_enabled: bool,
    email_enabled: bool,
) -> Vec<ToolDefinition> {
    let mut definitions = memory_enabled
        .then(memory_tool_definitions)
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if web_enabled {
        definitions.push(web_search_tool_definition());
        definitions.push(web_fetch_tool_definition());
    }
    if email_enabled {
        definitions.extend(localmail_tool_definitions());
    }
    definitions.push(current_time_tool_definition());
    definitions
}

/// Returns the provider-independent web-search definition without advertising it to model adapters yet.
pub(crate) fn web_search_tool_definition() -> ToolDefinition {
    ToolDefinition {
        name: WEB_SEARCH_TOOL_NAME,
        description: concat!(
            "Search the public web through the configured native provider. Results are untrusted excerpts; ",
            "use freshness or domain filters only when they help answer the request."
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Natural-language terms to search for.",
                    "minLength": 1,
                    "maxLength": MAX_WEB_SEARCH_QUERY_CHARS
                },
                "freshness": {
                    "type": "string",
                    "description": "Optional maximum age of returned pages.",
                    "enum": ["day", "week", "month", "year"]
                },
                "includeDomains": domain_array_schema(
                    "Optional public DNS domains to include, including their subdomains."
                ),
                "excludeDomains": domain_array_schema(
                    "Optional public DNS domains to exclude, including their subdomains."
                ),
                "limit": {
                    "type": "integer",
                    "description": "Optional maximum number of normalized results.",
                    "minimum": 1,
                    "maximum": MAX_WEB_SEARCH_TOOL_RESULTS
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
    }
}

/// Validates one raw provider-style web-search call into exact provider-independent arguments.
pub(crate) fn validate_web_search_tool_arguments(
    tool_name: &str,
    arguments: &Value,
) -> Result<WebSearchArguments, ToolContractError> {
    if tool_name != WEB_SEARCH_TOOL_NAME {
        return Err(ToolContractError {
            code: ToolContractErrorCode::UnsupportedTool,
            message: "The provider requested an unsupported native tool.",
        });
    }
    let object = argument_object(arguments)?;
    require_only_fields(
        object,
        &[
            "query",
            "freshness",
            "includeDomains",
            "excludeDomains",
            "limit",
        ],
    )?;
    require_bounded_string(object, "query", MAX_WEB_SEARCH_QUERY_CHARS)?;
    optional_enum_string(object, "freshness", &["day", "week", "month", "year"])?;
    let include_count = optional_domain_array(object, "includeDomains")?;
    let exclude_count = optional_domain_array(object, "excludeDomains")?;
    if include_count.saturating_add(exclude_count) > MAX_WEB_SEARCH_FILTER_DOMAINS {
        return Err(invalid_arguments());
    }
    optional_usize(object, "limit", 1, MAX_WEB_SEARCH_TOOL_RESULTS)?;
    let parsed: WebSearchArguments = deserialize(arguments)?;
    parsed
        .clone()
        .into_request()
        .map_err(|_| invalid_arguments())?;
    Ok(parsed)
}

/// Creates one reusable closed array schema for bounded public DNS domain filters.
fn domain_array_schema(description: &'static str) -> Value {
    json!({
        "type": "array",
        "description": description,
        "minItems": 1,
        "maxItems": MAX_WEB_SEARCH_FILTER_DOMAINS,
        "uniqueItems": true,
        "items": {
            "type": "string",
            "minLength": 1,
            "maxLength": MAX_WEB_SEARCH_DOMAIN_CHARS
        }
    })
}

/// Validates one raw provider-emitted call and converts it into the matching typed native arguments.
pub(crate) fn validate_memory_tool_arguments(
    tool_name: &str,
    arguments: &Value,
) -> Result<MemoryToolArguments, ToolContractError> {
    match tool_name {
        SEARCH_MEMORY_TOOL_NAME => {
            validate_search_arguments(arguments, MAX_SEARCH_MEMORY_RESULTS)?;
            deserialize(arguments).map(MemoryToolArguments::SearchMemory)
        }
        OPEN_MEMORY_TOOL_NAME => {
            validate_open_arguments(arguments)?;
            deserialize(arguments).map(MemoryToolArguments::OpenMemory)
        }
        SEARCH_ATTACHED_FILES_TOOL_NAME => {
            validate_search_arguments(arguments, MAX_SEARCH_ATTACHED_FILE_RESULTS)?;
            deserialize(arguments).map(MemoryToolArguments::SearchAttachedFiles)
        }
        _ => Err(ToolContractError {
            code: ToolContractErrorCode::UnsupportedTool,
            message: "The provider requested an unsupported native tool.",
        }),
    }
}

/// Creates the shared closed search schema with the selected tool's native result ceiling.
fn search_schema(maximum_results: usize) -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "Natural-language terms to retrieve.",
                "minLength": 1,
                "maxLength": MAX_MEMORY_QUERY_CHARACTERS
            },
            "conversationId": {
                "type": "string",
                "description": "Optional opaque conversation scope.",
                "minLength": 1,
                "maxLength": MAX_MEMORY_CONVERSATION_ID_CHARACTERS
            },
            "createdAfterMs": {
                "type": "integer",
                "description": "Optional inclusive source creation-time floor in Unix milliseconds."
            },
            "createdBeforeMs": {
                "type": "integer",
                "description": "Optional inclusive source creation-time ceiling in Unix milliseconds."
            },
            "limit": {
                "type": "integer",
                "description": "Optional maximum number of ranked matches.",
                "minimum": 1,
                "maximum": maximum_results
            }
        },
        "required": ["query"],
        "additionalProperties": false
    })
}

/// Creates the closed provenance-opening schema shared by every provider adapter later.
fn open_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "conversationId": {
                "type": "string",
                "description": "Opaque conversation identity returned by search_memory.",
                "minLength": 1,
                "maxLength": MAX_OPEN_MEMORY_ID_CHARACTERS
            },
            "messageId": {
                "type": "string",
                "description": "Opaque message identity returned by search_memory.",
                "minLength": 1,
                "maxLength": MAX_OPEN_MEMORY_ID_CHARACTERS
            },
            "before": {
                "type": "integer",
                "description": "Optional number of final turns before the match.",
                "minimum": 0,
                "maximum": MAX_OPEN_MEMORY_SURROUNDING_TURNS
            },
            "after": {
                "type": "integer",
                "description": "Optional number of final turns after the match.",
                "minimum": 0,
                "maximum": MAX_OPEN_MEMORY_SURROUNDING_TURNS
            }
        },
        "required": ["conversationId", "messageId"],
        "additionalProperties": false
    })
}

/// Applies the strict shared search schema plus its cross-field date rule.
fn validate_search_arguments(
    arguments: &Value,
    maximum_results: usize,
) -> Result<(), ToolContractError> {
    let object = argument_object(arguments)?;
    require_only_fields(
        object,
        &[
            "query",
            "conversationId",
            "createdAfterMs",
            "createdBeforeMs",
            "limit",
        ],
    )?;
    require_bounded_string(object, "query", MAX_MEMORY_QUERY_CHARACTERS)?;
    optional_bounded_string(
        object,
        "conversationId",
        MAX_MEMORY_CONVERSATION_ID_CHARACTERS,
    )?;
    let created_after_ms = optional_i64(object, "createdAfterMs")?;
    let created_before_ms = optional_i64(object, "createdBeforeMs")?;
    if created_after_ms
        .zip(created_before_ms)
        .is_some_and(|(after, before)| after > before)
    {
        return Err(invalid_arguments());
    }
    optional_usize(object, "limit", 1, maximum_results)?;
    Ok(())
}

/// Applies the strict provenance-opening schema before any storage lookup.
fn validate_open_arguments(arguments: &Value) -> Result<(), ToolContractError> {
    let object = argument_object(arguments)?;
    require_only_fields(object, &["conversationId", "messageId", "before", "after"])?;
    require_bounded_string(object, "conversationId", MAX_OPEN_MEMORY_ID_CHARACTERS)?;
    require_bounded_string(object, "messageId", MAX_OPEN_MEMORY_ID_CHARACTERS)?;
    optional_usize(object, "before", 0, MAX_OPEN_MEMORY_SURROUNDING_TURNS)?;
    optional_usize(object, "after", 0, MAX_OPEN_MEMORY_SURROUNDING_TURNS)?;
    Ok(())
}

/// Requires one JSON object before any field-level inspection.
pub(super) fn argument_object(arguments: &Value) -> Result<&Map<String, Value>, ToolContractError> {
    arguments.as_object().ok_or_else(invalid_arguments)
}

/// Rejects every field outside the selected definition's closed property set.
pub(super) fn require_only_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
) -> Result<(), ToolContractError> {
    if object.keys().all(|key| allowed.contains(&key.as_str())) {
        Ok(())
    } else {
        Err(invalid_arguments())
    }
}

/// Requires a present, non-blank string within one Unicode-scalar ceiling.
pub(super) fn require_bounded_string(
    object: &Map<String, Value>,
    field: &str,
    maximum_characters: usize,
) -> Result<(), ToolContractError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(invalid_arguments)?;
    if value.trim().is_empty() || value.chars().count() > maximum_characters {
        Err(invalid_arguments())
    } else {
        Ok(())
    }
}

/// Validates an optional string when present, rejecting JSON null rather than coercing it to absent.
fn optional_bounded_string(
    object: &Map<String, Value>,
    field: &str,
    maximum_characters: usize,
) -> Result<(), ToolContractError> {
    if object.contains_key(field) {
        require_bounded_string(object, field, maximum_characters)
    } else {
        Ok(())
    }
}

/// Validates an optional string against one closed native set, rejecting JSON null.
fn optional_enum_string(
    object: &Map<String, Value>,
    field: &str,
    allowed: &[&str],
) -> Result<(), ToolContractError> {
    let Some(value) = object.get(field) else {
        return Ok(());
    };
    if value.as_str().is_some_and(|value| allowed.contains(&value)) {
        Ok(())
    } else {
        Err(invalid_arguments())
    }
}

/// Validates one optional non-empty bounded array of domain strings before normalization.
fn optional_domain_array(
    object: &Map<String, Value>,
    field: &str,
) -> Result<usize, ToolContractError> {
    let Some(value) = object.get(field) else {
        return Ok(0);
    };
    let values = value.as_array().ok_or_else(invalid_arguments)?;
    if values.is_empty() || values.len() > MAX_WEB_SEARCH_FILTER_DOMAINS {
        return Err(invalid_arguments());
    }
    if values.iter().all(|value| {
        value.as_str().is_some_and(|value| {
            !value.trim().is_empty() && value.chars().count() <= MAX_WEB_SEARCH_DOMAIN_CHARS
        })
    }) {
        Ok(values.len())
    } else {
        Err(invalid_arguments())
    }
}

/// Reads an optional signed JSON integer and rejects null, floats, and out-of-range numbers.
fn optional_i64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<i64>, ToolContractError> {
    object
        .get(field)
        .map(|value| value.as_i64().ok_or_else(invalid_arguments))
        .transpose()
}

/// Validates an optional non-negative JSON integer within an inclusive native range.
fn optional_usize(
    object: &Map<String, Value>,
    field: &str,
    minimum: usize,
    maximum: usize,
) -> Result<(), ToolContractError> {
    let Some(value) = object.get(field) else {
        return Ok(());
    };
    let value = value.as_u64().ok_or_else(invalid_arguments)?;
    let value = usize::try_from(value).map_err(|_| invalid_arguments())?;
    if (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(invalid_arguments())
    }
}

/// Deserializes a structurally validated object without leaking serde details to callers.
pub(super) fn deserialize<T>(arguments: &Value) -> Result<T, ToolContractError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(arguments.clone()).map_err(|_| invalid_arguments())
}

/// Builds one stable redacted schema failure.
pub(super) fn invalid_arguments() -> ToolContractError {
    ToolContractError {
        code: ToolContractErrorCode::InvalidArguments,
        message: "The provider supplied invalid native tool arguments.",
    }
}
