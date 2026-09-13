//! Private local image-worker contracts kept separate from the approved Python tool runtime.

// The protocol intentionally lands before any process manager or worker implementation consumes it.
#[allow(dead_code, unused_imports)]
pub(crate) mod protocol;
// The lifecycle policy is intentionally transport-free until the next bounded worker slice.
#[allow(dead_code)]
pub(crate) mod manager;
