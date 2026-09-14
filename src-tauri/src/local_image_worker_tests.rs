//! Contract tests for the versioned private local image-worker protocol.

use crate::local_image_worker::protocol::{
    CURRENT_PROTOCOL_VERSION, FrameDecoder, HostMessage, MAX_FRAME_BYTES, ModelLocation,
    ProtocolError, WorkerCapabilities, WorkerFailureCode, WorkerMessage, WorkerOperation,
    WorkerOutput, WorkerProgressStage, WorkerProtocolSession, WorkerResult, decode_host_payload,
    decode_worker_payload, encode_host_frame, encode_worker_frame,
};

fn model_location() -> ModelLocation {
    ModelLocation {
        model_id: "Qwen/Qwen-Image-2512".into(),
        model_revision: "0123456789abcdef".into(),
        model_directory: test_model_directory().into(),
    }
}

#[cfg(not(target_os = "windows"))]
fn test_model_directory() -> &'static str {
    "/private/app-cache/qwen-image-2512"
}

#[cfg(target_os = "windows")]
fn test_model_directory() -> &'static str {
    r"C:\app-cache\qwen-image-2512"
}

fn generate_message() -> HostMessage {
    HostMessage::Generate {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: "generation-1".into(),
        model_id: "Qwen/Qwen-Image-2512".into(),
        prompt: "A quiet lighthouse at blue hour".into(),
        width: 1_024,
        height: 1_024,
        count: 1,
        seed: Some(42),
    }
}

fn completed_result() -> WorkerMessage {
    WorkerMessage::Result {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: "generation-1".into(),
        operation: WorkerOperation::Generate,
        result: WorkerResult::Completed {
            outputs: vec![WorkerOutput {
                output_name: "output-0.png".into(),
                width: 1_024,
                height: 1_024,
                seed: Some(42),
            }],
        },
    }
}

#[test]
fn round_trips_every_host_message_through_length_delimited_frames() {
    let messages = vec![
        HostMessage::Hello {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            client_version: env!("CARGO_PKG_VERSION").into(),
        },
        HostMessage::Load {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "load-1".into(),
            model: model_location(),
        },
        generate_message(),
        HostMessage::Cancel {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "generation-1".into(),
        },
        HostMessage::Shutdown {
            protocol_version: CURRENT_PROTOCOL_VERSION,
        },
    ];

    for message in messages {
        let encoded = encode_host_frame(&message).expect("valid host frame");
        let declared = u32::from_be_bytes(encoded[..4].try_into().unwrap()) as usize;
        assert_eq!(declared, encoded.len() - 4);
        assert_eq!(decode_host_payload(&encoded[4..]).unwrap(), message);
    }
}

#[test]
fn round_trips_every_worker_message_through_length_delimited_frames() {
    let messages = vec![
        WorkerMessage::Hello {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            worker_version: "0.1.0".into(),
        },
        WorkerMessage::Capabilities {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            capabilities: WorkerCapabilities {
                runtime_id: "mlx-gen".into(),
                generation: true,
                supports_seed: true,
                max_outputs: 1,
                max_pixels: 4_194_304,
            },
        },
        WorkerMessage::Progress {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "generation-1".into(),
            stage: WorkerProgressStage::Denoising,
            completed_steps: 3,
            total_steps: 20,
        },
        completed_result(),
    ];

    for message in messages {
        let encoded = encode_worker_frame(&message).expect("valid worker frame");
        assert_eq!(decode_worker_payload(&encoded[4..]).unwrap(), message);
    }
}

#[test]
fn frame_decoder_accepts_fragmented_and_consecutive_frames() {
    let first = encode_host_frame(&generate_message()).unwrap();
    let second = encode_host_frame(&HostMessage::Shutdown {
        protocol_version: CURRENT_PROTOCOL_VERSION,
    })
    .unwrap();
    let split = first.len() / 2;
    let mut decoder = FrameDecoder::new();

    assert!(decoder.push(&first[..split]).unwrap().is_empty());
    let mut tail = first[split..].to_vec();
    tail.extend_from_slice(&second);
    let payloads = decoder.push(&tail).unwrap();

    assert_eq!(payloads.len(), 2);
    assert_eq!(
        decode_host_payload(&payloads[0]).unwrap(),
        generate_message()
    );
    assert!(matches!(
        decode_host_payload(&payloads[1]).unwrap(),
        HostMessage::Shutdown { .. }
    ));
    assert_eq!(decoder.finish(), Ok(()));
}

#[test]
fn rejects_unknown_messages_fields_and_unsupported_versions() {
    for value in [
        serde_json::json!({"type":"launch","protocolVersion":1}),
        serde_json::json!({"type":"shutdown","protocolVersion":1,"extra":true}),
    ] {
        assert_eq!(
            decode_host_payload(&serde_json::to_vec(&value).unwrap()),
            Err(ProtocolError::MalformedMessage)
        );
    }
    let unsupported = serde_json::json!({"type":"shutdown","protocolVersion":2});
    assert_eq!(
        decode_host_payload(&serde_json::to_vec(&unsupported).unwrap()),
        Err(ProtocolError::UnsupportedVersion)
    );
}

#[test]
fn rejects_malformed_truncated_and_oversized_frames() {
    let mut decoder = FrameDecoder::new();
    assert_eq!(
        decoder.push(&[0, 0, 0, 0]),
        Err(ProtocolError::MalformedFrame)
    );

    let mut decoder = FrameDecoder::new();
    decoder.push(&[0, 0, 0, 8, b'{']).unwrap();
    assert_eq!(decoder.finish(), Err(ProtocolError::TruncatedFrame));

    let mut decoder = FrameDecoder::new();
    assert_eq!(
        decoder.push(&u32::MAX.to_be_bytes()),
        Err(ProtocolError::FrameTooLarge)
    );
    assert_eq!(MAX_FRAME_BYTES, 1_048_576);
}

#[test]
fn rejects_invalid_generation_bounds_and_output_names() {
    let invalid = HostMessage::Generate {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: "generation-1".into(),
        model_id: "Qwen/Qwen-Image-2512".into(),
        prompt: "image".into(),
        width: 0,
        height: 1_024,
        count: 1,
        seed: None,
    };
    assert_eq!(
        encode_host_frame(&invalid),
        Err(ProtocolError::InvalidField)
    );

    let invalid = WorkerMessage::Result {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: "generation-1".into(),
        operation: WorkerOperation::Generate,
        result: WorkerResult::Completed {
            outputs: vec![WorkerOutput {
                output_name: "../escaped.png".into(),
                width: 1_024,
                height: 1_024,
                seed: None,
            }],
        },
    };
    assert_eq!(
        encode_worker_frame(&invalid),
        Err(ProtocolError::InvalidField)
    );
}

#[test]
fn rejects_relative_model_directories_and_payloads_over_the_frame_limit() {
    let invalid = HostMessage::Load {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: "load-1".into(),
        model: ModelLocation {
            model_directory: "relative/model".into(),
            ..model_location()
        },
    };
    assert_eq!(
        encode_host_frame(&invalid),
        Err(ProtocolError::InvalidField)
    );
    assert_eq!(
        decode_host_payload(&vec![b' '; MAX_FRAME_BYTES + 1]),
        Err(ProtocolError::FrameTooLarge)
    );
}

#[test]
fn rejects_absolute_path_syntax_from_a_different_target_platform() {
    #[cfg(not(target_os = "windows"))]
    let foreign_path = r"C:\models\qwen-image-2512";
    #[cfg(target_os = "windows")]
    let foreign_path = "/private/app-cache/qwen-image-2512";
    let invalid = HostMessage::Load {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        request_id: "load-1".into(),
        model: ModelLocation {
            model_directory: foreign_path.into(),
            ..model_location()
        },
    };

    assert_eq!(
        encode_host_frame(&invalid),
        Err(ProtocolError::InvalidField)
    );
}

#[test]
fn rejects_path_shaped_or_sensitive_worker_errors() {
    for message in [
        "failed at /Users/example/model/config.json",
        r"failed at C:\models\config.json",
        "Bearer secret-token was rejected",
        "request used api_key=secret",
    ] {
        let failed = WorkerMessage::Result {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            request_id: "generation-1".into(),
            operation: WorkerOperation::Generate,
            result: WorkerResult::Failed {
                code: WorkerFailureCode::Internal,
                message: Some(message.into()),
            },
        };
        assert_eq!(
            encode_worker_frame(&failed),
            Err(ProtocolError::UnsafeErrorDetail)
        );
    }
}

#[test]
fn session_rejects_duplicate_terminal_results() {
    let result = completed_result();
    let mut session = WorkerProtocolSession::new();

    assert_eq!(session.accept(&result), Ok(()));
    assert_eq!(
        session.accept(&result),
        Err(ProtocolError::DuplicateTerminalResult)
    );
}

#[test]
fn closed_result_schema_rejects_unknown_terminal_fields() {
    let value = serde_json::json!({
        "type": "result",
        "protocolVersion": 1,
        "requestId": "generation-1",
        "operation": "generate",
        "result": {"status": "cancelled", "unexpected": true}
    });
    assert_eq!(
        decode_worker_payload(&serde_json::to_vec(&value).unwrap()),
        Err(ProtocolError::MalformedMessage)
    );
}
