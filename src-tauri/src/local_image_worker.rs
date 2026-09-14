//! Private local image-worker contracts kept separate from the approved Python tool runtime.

// The protocol intentionally lands before any runtime-specific worker implementation consumes it.
#[allow(dead_code)]
#[path = "local_image_worker/manager.rs"]
pub(crate) mod manager;
#[allow(dead_code)]
#[path = "local_image_worker/model_acquisition.rs"]
pub(crate) mod model_acquisition;
#[allow(dead_code)]
#[path = "local_image_worker/model_cache.rs"]
pub(crate) mod model_cache;
#[allow(dead_code)]
#[path = "local_image_worker/model_download.rs"]
pub(crate) mod model_download;
#[allow(dead_code)]
#[path = "local_image_worker/model_package.rs"]
pub(crate) mod model_package;
#[allow(dead_code, unused_imports)]
#[path = "local_image_worker/protocol.rs"]
pub(crate) mod protocol;
#[cfg(feature = "local-image-runtime-proof")]
#[allow(dead_code)]
#[path = "local_image_worker/runtime_proof.rs"]
pub(crate) mod runtime_proof;
#[allow(dead_code)]
#[path = "local_image_worker/transport.rs"]
pub(crate) mod transport;
