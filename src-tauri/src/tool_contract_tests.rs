//! Provider-independent native tool definition and argument-validation tests.

use serde_json::{Value, json};

use crate::{
    storage::{OpenMemoryArguments, SearchAttachedFilesArguments, SearchMemoryArguments},
    tool_contract::{
        CURRENT_TIME_TOOL_NAME, MemoryToolArguments, ToolContractErrorCode,
        current_time_tool_definition, enabled_native_tool_definitions, memory_tool_definitions,
        validate_current_time_tool_arguments, validate_memory_tool_arguments,
        validate_web_fetch_tool_arguments, validate_web_search_tool_arguments,
        web_fetch_tool_definition, web_search_tool_definition,
    },
    web_fetch::WebFetchArguments,
    web_search::{WebSearchArguments, WebSearchFreshness},
};

#[test]
fn selects_memory_web_and_clock_definitions_independently_in_stable_order() {
    let names = |memory_enabled, web_enabled| {
        enabled_native_tool_definitions(memory_enabled, web_enabled)
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>()
    };

    assert_eq!(names(false, false), ["current_time"]);
    assert_eq!(
        names(true, false),
        [
            "search_memory",
            "open_memory",
            "search_attached_files",
            "current_time"
        ]
    );
    assert_eq!(
        names(false, true),
        ["web_search", "web_fetch", "current_time"]
    );
    assert_eq!(names(true, true).len(), 6);
}

#[test]
fn publishes_and_validates_one_closed_zero_argument_clock_schema() {
    let definition = serde_json::to_value(current_time_tool_definition())
        .expect("clock definition should serialize");

    assert_eq!(definition["name"], json!(CURRENT_TIME_TOOL_NAME));
    assert_eq!(definition["inputSchema"]["type"], json!("object"));
    assert_eq!(definition["inputSchema"]["properties"], json!({}));
    assert_eq!(definition["inputSchema"]["required"], json!([]));
    assert_eq!(
        definition["inputSchema"]["additionalProperties"],
        json!(false)
    );
    validate_current_time_tool_arguments(CURRENT_TIME_TOOL_NAME, &json!({}))
        .expect("an exact empty object should validate");
}

#[test]
fn rejects_nonempty_or_nonobject_clock_arguments_without_reflecting_them() {
    for (name, arguments, expected) in [
        (
            CURRENT_TIME_TOOL_NAME,
            json!(null),
            ToolContractErrorCode::InvalidArguments,
        ),
        (
            CURRENT_TIME_TOOL_NAME,
            json!([]),
            ToolContractErrorCode::InvalidArguments,
        ),
        (
            CURRENT_TIME_TOOL_NAME,
            json!({"timezone": "Australia/Perth"}),
            ToolContractErrorCode::InvalidArguments,
        ),
        (
            "host_time",
            json!({}),
            ToolContractErrorCode::UnsupportedTool,
        ),
    ] {
        let error = validate_current_time_tool_arguments(name, &arguments)
            .expect_err("invalid clock arguments should fail closed");
        assert_eq!(error.code, expected);
        assert!(!error.message.contains(&arguments.to_string()));
        assert!(!error.message.contains("Australia/Perth"));
    }
}

/// Returns the definition with one stable native name.
fn definition(name: &str) -> Value {
    memory_tool_definitions()
        .into_iter()
        .find(|definition| definition.name == name)
        .map(|definition| serde_json::to_value(definition).expect("definition should serialize"))
        .expect("named memory tool should exist")
}

#[test]
fn publishes_one_closed_provider_independent_web_fetch_schema() {
    let definition = serde_json::to_value(web_fetch_tool_definition())
        .expect("web-fetch definition should serialize");

    assert_eq!(definition["name"], json!("web_fetch"));
    assert_eq!(definition["inputSchema"]["type"], json!("object"));
    assert_eq!(
        definition["inputSchema"]["additionalProperties"],
        json!(false)
    );
    assert_eq!(definition["inputSchema"]["required"], json!(["url"]));
    assert_eq!(
        definition["inputSchema"]["properties"]["url"]["maxLength"],
        json!(4096)
    );
}

#[test]
fn validates_web_fetch_into_an_exact_public_url_argument() {
    let arguments = validate_web_fetch_tool_arguments(
        "web_fetch",
        &json!({"url": "https://www.iana.org/release#notes"}),
    )
    .expect("web-fetch arguments should validate");

    assert_eq!(
        arguments,
        WebFetchArguments {
            url: "https://www.iana.org/release#notes".into(),
        }
    );
}

#[test]
fn rejects_invalid_web_fetch_shapes_without_reflecting_arguments() {
    for (tool_name, arguments) in [
        ("web_search", json!({"url": "https://www.iana.org"})),
        ("web_fetch", json!([])),
        ("web_fetch", json!({})),
        ("web_fetch", json!({"url": null})),
        ("web_fetch", json!({"url": "file:///private/secret"})),
        ("web_fetch", json!({"url": "http://127.0.0.1/private"})),
        (
            "web_fetch",
            json!({"url": "https://user:secret@www.iana.org"}),
        ),
        (
            "web_fetch",
            json!({"url": "https://www.iana.org", "headers": {}}),
        ),
    ] {
        let error = validate_web_fetch_tool_arguments(tool_name, &arguments)
            .expect_err("invalid web-fetch arguments should fail");
        assert!(!error.message.contains(&arguments.to_string()));
    }
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
fn publishes_one_closed_provider_independent_web_search_schema() {
    let definition = serde_json::to_value(web_search_tool_definition())
        .expect("web-search definition should serialize");

    assert_eq!(definition["name"], json!("web_search"));
    assert_eq!(definition["inputSchema"]["type"], json!("object"));
    assert_eq!(
        definition["inputSchema"]["additionalProperties"],
        json!(false)
    );
    assert_eq!(definition["inputSchema"]["required"], json!(["query"]));
    assert_eq!(
        definition["inputSchema"]["properties"]["freshness"]["enum"],
        json!(["day", "week", "month", "year"])
    );
    assert_eq!(
        definition["inputSchema"]["properties"]["includeDomains"]["maxItems"],
        json!(5)
    );
    assert_eq!(
        definition["inputSchema"]["properties"]["excludeDomains"]["maxItems"],
        json!(5)
    );
    assert_eq!(
        definition["inputSchema"]["properties"]["limit"]["maximum"],
        json!(10)
    );

    let memory_names = memory_tool_definitions()
        .into_iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    assert_eq!(
        memory_names,
        ["search_memory", "open_memory", "search_attached_files"]
    );
}

#[test]
fn validates_web_search_freshness_and_domain_filters_into_exact_arguments() {
    let arguments = validate_web_search_tool_arguments(
        "web_search",
        &json!({
            "query": "native Rust boundaries",
            "freshness": "week",
            "includeDomains": ["docs.rs", "rust-lang.org"],
            "excludeDomains": ["forum.example"],
            "limit": 7
        }),
    )
    .expect("web-search arguments should validate");

    assert_eq!(
        arguments,
        WebSearchArguments {
            query: "native Rust boundaries".into(),
            freshness: Some(WebSearchFreshness::Week),
            include_domains: vec!["docs.rs".into(), "rust-lang.org".into()],
            exclude_domains: vec!["forum.example".into()],
            limit: Some(7),
        }
    );
}

#[test]
fn rejects_invalid_web_search_shapes_without_reflecting_arguments() {
    let cases = [
        ("web_fetch", json!({"query": "private"})),
        ("web_search", json!([])),
        ("web_search", json!({})),
        ("web_search", json!({"query": "private", "freshness": null})),
        (
            "web_search",
            json!({"query": "private", "freshness": "hour"}),
        ),
        (
            "web_search",
            json!({"query": "private", "includeDomains": []}),
        ),
        (
            "web_search",
            json!({"query": "private", "includeDomains": ["https://example.com/private"]}),
        ),
        (
            "web_search",
            json!({"query": "private", "excludeDomains": ["example.com", "EXAMPLE.com"]}),
        ),
        (
            "web_search",
            json!({"query": "private", "includeDomains": ["example.com"], "excludeDomains": ["example.com"]}),
        ),
        ("web_search", json!({"query": "private", "limit": 0})),
        ("web_search", json!({"query": "private", "limit": 11})),
        (
            "web_search",
            json!({"query": "private", "unknown": "secret"}),
        ),
    ];

    for (tool_name, arguments) in cases {
        let error = validate_web_search_tool_arguments(tool_name, &arguments)
            .expect_err("invalid web-search arguments should fail");
        assert!(!error.message.contains(&arguments.to_string()));
    }
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
