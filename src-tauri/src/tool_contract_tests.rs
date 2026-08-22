//! Provider-independent native tool definition and argument-validation tests.

use serde_json::{Value, json};

use crate::{
    storage::{OpenMemoryArguments, SearchAttachedFilesArguments, SearchMemoryArguments},
    tool_contract::{
        MemoryToolArguments, ToolContractErrorCode, memory_tool_definitions,
        validate_memory_tool_arguments,
    },
};

/// Returns the definition with one stable native name.
fn definition(name: &str) -> Value {
    memory_tool_definitions()
        .into_iter()
        .find(|definition| definition.name == name)
        .map(|definition| serde_json::to_value(definition).expect("definition should serialize"))
        .expect("named memory tool should exist")
}

#[test]
fn publishes_three_closed_provider_independent_memory_schemas() {
    let definitions = memory_tool_definitions();
    assert_eq!(
        definitions
            .iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>(),
        ["search_memory", "open_memory", "search_attached_files"]
    );

    for definition in definitions {
        assert!(!definition.description.trim().is_empty());
        assert_eq!(definition.input_schema["type"], json!("object"));
        assert_eq!(
            definition.input_schema["additionalProperties"],
            json!(false)
        );
    }

    let search = definition("search_memory");
    assert_eq!(search["inputSchema"]["required"], json!(["query"]));
    assert_eq!(
        search["inputSchema"]["properties"]["query"]["maxLength"],
        json!(200)
    );
    assert_eq!(
        search["inputSchema"]["properties"]["limit"]["maximum"],
        json!(10)
    );

    let open = definition("open_memory");
    assert_eq!(
        open["inputSchema"]["required"],
        json!(["conversationId", "messageId"])
    );
    assert_eq!(
        open["inputSchema"]["properties"]["before"]["maximum"],
        json!(3)
    );
    assert_eq!(
        open["inputSchema"]["properties"]["after"]["maximum"],
        json!(3)
    );

    assert_eq!(
        definition("search_attached_files")["inputSchema"],
        search["inputSchema"]
    );
    let serialized =
        serde_json::to_string(&memory_tool_definitions()).expect("definitions should serialize");
    for forbidden in ["path", "hash", "embedding", "automatic"] {
        assert!(!serialized.to_lowercase().contains(forbidden));
    }
}

#[test]
fn validates_raw_json_into_exact_typed_memory_arguments() {
    let search = validate_memory_tool_arguments(
        "search_memory",
        &json!({
            "query": "native boundaries",
            "conversationId": "conversation-id",
            "createdAfterMs": 1,
            "createdBeforeMs": 2,
            "limit": 10
        }),
    )
    .expect("search_memory arguments should validate");
    assert_eq!(
        search,
        MemoryToolArguments::SearchMemory(SearchMemoryArguments {
            query: "native boundaries".into(),
            conversation_id: Some("conversation-id".into()),
            created_after_ms: Some(1),
            created_before_ms: Some(2),
            limit: Some(10),
        })
    );

    let open = validate_memory_tool_arguments(
        "open_memory",
        &json!({
            "conversationId": "conversation-id",
            "messageId": "message-id",
            "before": 0,
            "after": 3
        }),
    )
    .expect("open_memory arguments should validate");
    assert_eq!(
        open,
        MemoryToolArguments::OpenMemory(OpenMemoryArguments {
            conversation_id: "conversation-id".into(),
            message_id: "message-id".into(),
            before: Some(0),
            after: Some(3),
        })
    );

    let files =
        validate_memory_tool_arguments("search_attached_files", &json!({"query": "field notes"}))
            .expect("search_attached_files arguments should validate");
    assert_eq!(
        files,
        MemoryToolArguments::SearchAttachedFiles(SearchAttachedFilesArguments {
            query: "field notes".into(),
            ..SearchAttachedFilesArguments::default()
        })
    );
}

#[test]
fn rejects_unknown_tools_and_structurally_invalid_json() {
    let unsupported = validate_memory_tool_arguments("web_fetch", &json!({})).unwrap_err();
    assert_eq!(unsupported.code, ToolContractErrorCode::UnsupportedTool);

    for (tool_name, arguments) in [
        ("search_memory", json!(["not", "an", "object"])),
        ("search_memory", json!({})),
        (
            "search_memory",
            json!({"query": "north", "includePaths": true}),
        ),
        ("search_memory", json!({"query": 42})),
        ("search_memory", json!({"query": "north", "limit": 1.5})),
        ("search_memory", json!({"query": "north", "limit": null})),
        (
            "search_attached_files",
            json!({"query": "north", "conversationId": null}),
        ),
        ("open_memory", json!({"conversationId": "conversation-id"})),
        (
            "open_memory",
            json!({"conversationId": "conversation-id", "messageId": "message-id", "extra": null}),
        ),
    ] {
        let error = validate_memory_tool_arguments(tool_name, &arguments)
            .expect_err("malformed arguments should fail");
        assert_eq!(error.code, ToolContractErrorCode::InvalidArguments);
        assert!(!error.message.contains(&arguments.to_string()));
    }
}

#[test]
fn rejects_values_outside_declared_schema_bounds() {
    let cases = [
        ("search_memory", json!({"query": "   "})),
        ("search_memory", json!({"query": "x".repeat(201)})),
        (
            "search_memory",
            json!({"query": "north", "conversationId": "  "}),
        ),
        (
            "search_memory",
            json!({"query": "north", "conversationId": "x".repeat(129)}),
        ),
        ("search_memory", json!({"query": "north", "limit": 0})),
        ("search_memory", json!({"query": "north", "limit": 11})),
        ("search_memory", json!({"query": "north", "limit": -1})),
        (
            "search_attached_files",
            json!({"query": "north", "createdAfterMs": 2, "createdBeforeMs": 1}),
        ),
        (
            "open_memory",
            json!({"conversationId": " ", "messageId": "message-id"}),
        ),
        (
            "open_memory",
            json!({"conversationId": "conversation-id", "messageId": "x".repeat(129)}),
        ),
        (
            "open_memory",
            json!({"conversationId": "conversation-id", "messageId": "message-id", "before": 4}),
        ),
        (
            "open_memory",
            json!({"conversationId": "conversation-id", "messageId": "message-id", "after": 4}),
        ),
        (
            "open_memory",
            json!({"conversationId": "conversation-id", "messageId": "message-id", "after": -1}),
        ),
    ];

    for (tool_name, arguments) in cases {
        assert_eq!(
            validate_memory_tool_arguments(tool_name, &arguments)
                .expect_err("out-of-schema arguments should fail")
                .code,
            ToolContractErrorCode::InvalidArguments
        );
    }
}
