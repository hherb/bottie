//! Private local image-worker contracts kept separate from the approved Python tool runtime.

// The protocol intentionally lands before any runtime-specific worker implementation consumes it.
#[allow(dead_code)]
pub(crate) mod manager;
#[allow(dead_code)]
pub(crate) mod model_acquisition;
#[allow(dead_code)]
pub(crate) mod model_cache;
#[allow(dead_code)]
pub(crate) mod model_download;
#[allow(dead_code)]
pub(crate) mod model_package;
#[allow(dead_code, unused_imports)]
pub(crate) mod protocol;
#[allow(dead_code)]
pub(crate) mod transport;
