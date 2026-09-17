//! Hand-declared subset of `audio.cpp`'s C ABI (`include/audiocpp.h`).
//!
//! Only what the spike exercises is declared. The ABI's own rules apply: returned
//! `const char *` are borrowed until the handle that produced them is freed, and
//! every call returns a status rather than throwing.

use std::os::raw::{c_char, c_double, c_int, c_void};

pub type Status = c_int;
pub const AUDIOCPP_OK: Status = 0;

#[repr(C)]
pub struct Registry {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Model {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Session {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Request {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Result_ {
    _private: [u8; 0],
}

#[repr(C)]
pub struct ModelConfig {
    pub family: *const c_char,
    pub config: *const c_char,
    pub weight: *const c_char,
    pub model_spec_override: *const c_char,
}

#[repr(C)]
pub struct BackendConfig {
    pub backend: *const c_char,
    pub device: c_int,
    pub threads: c_int,
}

unsafe extern "C" {
    pub fn audiocpp_abi_version() -> u32;
    pub fn audiocpp_build_version() -> *const c_char;
    pub fn audiocpp_last_error() -> *const c_char;
    pub fn audiocpp_status_string(status: Status) -> *const c_char;

    pub fn audiocpp_registry_create(config_path: *const c_char, out: *mut *mut Registry) -> Status;
    pub fn audiocpp_registry_free(registry: *mut Registry);
    pub fn audiocpp_registry_family_count(registry: *const Registry) -> usize;
    pub fn audiocpp_registry_family(
        registry: *const Registry,
        index: usize,
        out_family: *mut *const c_char,
    ) -> Status;

    pub fn audiocpp_model_load(
        registry: *mut Registry,
        model_path: *const c_char,
        config: *const ModelConfig,
        options: *const c_void,
        out_model: *mut *mut Model,
    ) -> Status;
    pub fn audiocpp_model_free(model: *mut Model);

    pub fn audiocpp_session_create(
        model: *const Model,
        task: *const c_char,
        mode: *const c_char,
        backend_config: *const BackendConfig,
        options: *const c_void,
        out_session: *mut *mut Session,
    ) -> Status;
    pub fn audiocpp_session_free(session: *mut Session);
    pub fn audiocpp_session_run(
        session: *mut Session,
        request: *const Request,
        out_result: *mut *mut Result_,
    ) -> Status;

    pub fn audiocpp_request_create() -> *mut Request;
    pub fn audiocpp_request_free(request: *mut Request);
    pub fn audiocpp_request_set_text(
        request: *mut Request,
        text: *const c_char,
        language: *const c_char,
    ) -> Status;
    pub fn audiocpp_request_set_option(
        request: *mut Request,
        key: *const c_char,
        value: *const c_char,
    ) -> Status;

    pub fn audiocpp_result_free(result: *mut Result_);
    pub fn audiocpp_result_audio(
        result: *const Result_,
        out_samples: *mut *const f32,
        out_frames: *mut usize,
        out_rate: *mut c_int,
        out_channels: *mut c_int,
    ) -> Status;

    pub fn audiocpp_stream_policy(
        session: *const Session,
        out_input: *mut c_int,
        out_output: *mut c_int,
        out_preferred_chunk_samples: *mut i64,
        out_preferred_chunk_seconds: *mut c_double,
    ) -> Status;
}
