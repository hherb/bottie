//! Deterministic private-pipe fixture for local image-worker transport tests.

use std::{
    io::{Read, Write},
    process, thread,
    time::Duration,
};

#[allow(dead_code, unused_imports)]
#[path = "local_image_worker/protocol.rs"]
mod protocol;

use protocol::{
    CURRENT_PROTOCOL_VERSION, HostMessage, WorkerCapabilities, WorkerMessage, WorkerOperation,
    WorkerOutput, WorkerProgressStage, WorkerResult, decode_host_payload, encode_worker_frame,
};

const FIXTURE_SLEEP: Duration = Duration::from_secs(60);
const STDERR_FLOOD_BYTES: usize = 80 * 1_024;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "normal".into());
    if mode == "early-exit" {
        process::exit(17);
    }
    if std::env::var_os("PATH").is_some() {
        process::exit(18);
    }
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let hello = read_host_message(&mut input);
    if !matches!(hello, HostMessage::Hello { .. }) {
        process::exit(19);
    }
    match mode.as_str() {
        "hung-handshake" => thread::sleep(FIXTURE_SLEEP),
        "malformed" => write_malformed(&mut output),
        "stderr-flood" => flood_stderr(),
        "wrong-order" => write_message(&mut output, &capabilities_message()),
        "fragmented" => write_fragmented_handshake(&mut output),
        _ => write_handshake(&mut output),
    }
    if matches!(
        mode.as_str(),
        "hung-handshake" | "malformed" | "stderr-flood"
    ) {
        return;
    }
    run_commands(&mode, &mut input, &mut output);
}

fn run_commands(mode: &str, input: &mut impl Read, output: &mut impl Write) {
    loop {
        match read_host_message(input) {
            HostMessage::Load { request_id, .. } => write_message(
                output,
                &WorkerMessage::Result {
                    protocol_version: CURRENT_PROTOCOL_VERSION,
                    request_id,
                    operation: WorkerOperation::Load,
                    result: WorkerResult::Completed { outputs: vec![] },
                },
            ),
            HostMessage::Generate {
                request_id,
                width,
                height,
                seed,
                ..
            } => {
                if mode == "hung-operation" {
                    continue;
                }
                write_message(
                    output,
                    &WorkerMessage::Progress {
                        protocol_version: CURRENT_PROTOCOL_VERSION,
                        request_id: request_id.clone(),
                        stage: WorkerProgressStage::Denoising,
                        completed_steps: 1,
                        total_steps: 2,
                    },
                );
                if mode != "cooperative-cancel" && mode != "ignore-cancel" {
                    write_message(
                        output,
                        &WorkerMessage::Result {
                            protocol_version: CURRENT_PROTOCOL_VERSION,
                            request_id,
                            operation: WorkerOperation::Generate,
                            result: WorkerResult::Completed {
                                outputs: vec![WorkerOutput {
                                    output_name: "fixture-output.png".into(),
                                    width,
                                    height,
                                    seed,
                                }],
                            },
                        },
                    );
                }
            }
            HostMessage::Cancel { request_id, .. } if mode == "cooperative-cancel" => {
                write_message(
                    output,
                    &WorkerMessage::Result {
                        protocol_version: CURRENT_PROTOCOL_VERSION,
                        request_id,
                        operation: WorkerOperation::Generate,
                        result: WorkerResult::Cancelled {},
                    },
                );
            }
            HostMessage::Cancel { .. } if mode == "ignore-cancel" => {}
            HostMessage::Shutdown { .. } if mode == "hung-shutdown" => {
                thread::sleep(FIXTURE_SLEEP);
            }
            HostMessage::Shutdown { .. } => return,
            HostMessage::Hello { .. } | HostMessage::Cancel { .. } => process::exit(20),
        }
    }
}

fn read_host_message(input: &mut impl Read) -> HostMessage {
    let mut prefix = [0_u8; 4];
    input
        .read_exact(&mut prefix)
        .unwrap_or_else(|_| process::exit(21));
    let length = u32::from_be_bytes(prefix) as usize;
    let mut payload = vec![0_u8; length];
    input
        .read_exact(&mut payload)
        .unwrap_or_else(|_| process::exit(22));
    decode_host_payload(&payload).unwrap_or_else(|_| process::exit(23))
}

fn write_handshake(output: &mut impl Write) {
    write_message(output, &hello_message());
    write_message(output, &capabilities_message());
}

fn write_fragmented_handshake(output: &mut impl Write) {
    let mut frames = encode_worker_frame(&hello_message()).unwrap();
    frames.extend(encode_worker_frame(&capabilities_message()).unwrap());
    for chunk in frames.chunks(3) {
        output.write_all(chunk).unwrap();
        output.flush().unwrap();
    }
}

fn hello_message() -> WorkerMessage {
    WorkerMessage::Hello {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        worker_version: "fixture-1".into(),
    }
}

fn capabilities_message() -> WorkerMessage {
    WorkerMessage::Capabilities {
        protocol_version: CURRENT_PROTOCOL_VERSION,
        capabilities: WorkerCapabilities {
            runtime_id: "fixture-runtime".into(),
            generation: true,
            supports_seed: true,
            max_outputs: 1,
            max_pixels: 4_194_304,
        },
    }
}

fn write_message(output: &mut impl Write, message: &WorkerMessage) {
    output
        .write_all(&encode_worker_frame(message).unwrap())
        .unwrap();
    output.flush().unwrap();
}

fn write_malformed(output: &mut impl Write) {
    output.write_all(&2_u32.to_be_bytes()).unwrap();
    output.write_all(b"{}").unwrap();
    output.flush().unwrap();
}

fn flood_stderr() {
    let bytes = vec![b'x'; STDERR_FLOOD_BYTES];
    std::io::stderr().write_all(&bytes).unwrap();
    std::io::stderr().flush().unwrap();
    thread::sleep(FIXTURE_SLEEP);
}
