//! Shared exact package and TCP response fixtures for model downloader tests.

use std::{
    fs,
    future::Future,
    io::{self, Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

use crate::local_image_worker::{
    model_acquisition::{ModelAcquisition, ModelFileContract, ModelPackageManifest},
    model_download::{ModelDownloadLimits, ModelFileSource, ModelSourcePlan},
};

pub(super) const MODEL_BYTES: &[u8] = b"weights";
pub(super) const SOURCE_REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
pub(super) const SOURCE_ETAG: &str = "\"fixture-weights-v1\"";

#[derive(Clone)]
pub(super) struct FixtureResponse {
    pub(super) status: &'static str,
    pub(super) headers: Vec<(String, String)>,
    pub(super) first_body: Vec<u8>,
    pub(super) delayed_body: Vec<u8>,
    pub(super) delay: Duration,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn manifest() -> ModelPackageManifest {
    ModelPackageManifest {
        model_id: "Qwen/Qwen-Image-2512".into(),
        runtime_id: "fixture-runtime@0123456789abcdef".into(),
        license: "Apache-2.0".into(),
        source_revision: SOURCE_REVISION.into(),
        expected_disk_bytes: MODEL_BYTES.len() as u64,
        expected_memory_bytes: 20 * 1_024 * 1_024 * 1_024,
        files: vec![ModelFileContract {
            relative_path: "weights/model.bin".into(),
            byte_size: MODEL_BYTES.len() as u64,
            sha256: digest(MODEL_BYTES),
        }],
    }
}

pub(super) fn source_plan(root: &str) -> ModelSourcePlan {
    source_plan_with_etag(root, SOURCE_ETAG)
}

pub(super) fn source_plan_with_etag(root: &str, etag: &str) -> ModelSourcePlan {
    ModelSourcePlan::for_loopback_fixture(
        manifest(),
        root,
        SOURCE_REVISION,
        vec![ModelFileSource {
            relative_path: "weights/model.bin".into(),
            strong_etag: etag.into(),
        }],
    )
    .expect("fixture source plan should be valid")
}

pub(super) fn approved_acquisition() -> ModelAcquisition {
    let mut acquisition = ModelAcquisition::new(manifest()).unwrap();
    acquisition.begin_download().unwrap();
    acquisition
}

pub(super) fn limits(file_timeout: Duration) -> ModelDownloadLimits {
    ModelDownloadLimits::for_fixture(
        Duration::from_secs(1),
        file_timeout,
        Duration::from_secs(2),
        16,
        16,
        1,
    )
}

pub(super) fn temp_cache(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "bottie-model-download-{name}-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

pub(super) fn block_on<T>(future: impl Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("fixture runtime should build")
        .block_on(future)
}

pub(super) fn response(
    status: &'static str,
    content_length: usize,
    etag: &str,
    body: &[u8],
) -> FixtureResponse {
    FixtureResponse {
        status,
        headers: vec![
            ("Content-Length".into(), content_length.to_string()),
            ("ETag".into(), etag.into()),
        ],
        first_body: body.to_vec(),
        delayed_body: Vec::new(),
        delay: Duration::ZERO,
    }
}

pub(super) fn serve(
    response: FixtureResponse,
) -> (String, Arc<Mutex<String>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener should bind");
    let address = listener.local_addr().unwrap();
    let request_text = Arc::new(Mutex::new(String::new()));
    let captured = Arc::clone(&request_text);
    let server = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return;
                    }
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("fixture accept failed: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let mut request = [0_u8; 4_096];
        let count = stream.read(&mut request).unwrap_or(0);
        *captured.lock().unwrap() = String::from_utf8_lossy(&request[..count]).into_owned();
        let mut headers = format!("HTTP/1.1 {}\r\n", response.status);
        for (name, value) in response.headers {
            headers.push_str(&format!("{name}: {value}\r\n"));
        }
        headers.push_str("Connection: close\r\n\r\n");
        stream.write_all(headers.as_bytes()).unwrap();
        stream.write_all(&response.first_body).unwrap();
        stream.flush().unwrap();
        if !response.delay.is_zero() {
            thread::sleep(response.delay);
        }
        let _ = stream.write_all(&response.delayed_body);
    });
    (format!("http://{address}/models/"), request_text, server)
}
