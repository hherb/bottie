//! Field and relationship validation for closed local image-worker messages.

use std::path::{Component, Path};

use super::{
    CURRENT_PROTOCOL_VERSION, HostMessage, ModelLocation, ProtocolError, WorkerCapabilities,
    WorkerMessage, WorkerOperation, WorkerOutput, WorkerResult,
};

/// Maximum UTF-8 bytes accepted for an opaque request identifier.
const MAX_REQUEST_ID_BYTES: usize = 128;
/// Maximum UTF-8 bytes accepted for a model or runtime identity.
const MAX_IDENTITY_BYTES: usize = 256;
/// Maximum UTF-8 bytes accepted for a native-only model directory.
const MAX_MODEL_DIRECTORY_BYTES: usize = 4_096;
/// Maximum Unicode scalar count accepted for a generation prompt.
const MAX_PROMPT_CHARACTERS: usize = 4_096;
/// Maximum image outputs accepted from one local generation.
const MAX_OUTPUTS: u8 = 6;
/// Maximum dimension accepted on either image axis.
const MAX_IMAGE_AXIS: u32 = 4_096;
/// Maximum total pixels accepted for one local output.
const MAX_IMAGE_PIXELS: u64 = 4_194_304;
/// Maximum safe human-readable worker failure detail.
const MAX_ERROR_MESSAGE_BYTES: usize = 512;

pub(super) fn validate_host_message(message: &HostMessage) -> Result<(), ProtocolError> {
    match message {
        HostMessage::Hello {
            protocol_version,
            client_version,
        } => {
            validate_version(*protocol_version)?;
            validate_identity(client_version)
        }
        HostMessage::Load {
            protocol_version,
            request_id,
            model,
        } => {
            validate_version(*protocol_version)?;
            validate_request_id(request_id)?;
            validate_model_location(model)
        }
        HostMessage::Generate {
            protocol_version,
            request_id,
            model_id,
            prompt,
            width,
            height,
            count,
            ..
        } => {
            validate_version(*protocol_version)?;
            validate_request_id(request_id)?;
            validate_identity(model_id)?;
            validate_prompt(prompt)?;
            validate_image_bounds(*width, *height, *count)
        }
        HostMessage::Cancel {
            protocol_version,
            request_id,
        } => {
            validate_version(*protocol_version)?;
            validate_request_id(request_id)
        }
        HostMessage::Shutdown { protocol_version } => validate_version(*protocol_version),
    }
}

pub(super) fn validate_worker_message(message: &WorkerMessage) -> Result<(), ProtocolError> {
    match message {
        WorkerMessage::Hello {
            protocol_version,
            worker_version,
        } => {
            validate_version(*protocol_version)?;
            validate_identity(worker_version)
        }
        WorkerMessage::Capabilities {
            protocol_version,
            capabilities,
        } => {
            validate_version(*protocol_version)?;
            validate_capabilities(capabilities)
        }
        WorkerMessage::Progress {
            protocol_version,
            request_id,
            completed_steps,
            total_steps,
            ..
        } => {
            validate_version(*protocol_version)?;
            validate_request_id(request_id)?;
            if *total_steps == 0 || completed_steps > total_steps {
                return Err(ProtocolError::InvalidField);
            }
            Ok(())
        }
        WorkerMessage::Result {
            protocol_version,
            request_id,
            operation,
            result,
        } => {
            validate_version(*protocol_version)?;
            validate_request_id(request_id)?;
            validate_result(*operation, result)
        }
    }
}

fn validate_version(version: u16) -> Result<(), ProtocolError> {
    if version == CURRENT_PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(ProtocolError::UnsupportedVersion)
    }
}

fn validate_model_location(model: &ModelLocation) -> Result<(), ProtocolError> {
    validate_identity(&model.model_id)?;
    validate_identity(&model.model_revision)?;
    if !is_absolute_native_path(&model.model_directory)
        || model.model_directory.len() > MAX_MODEL_DIRECTORY_BYTES
        || model.model_directory.chars().any(char::is_control)
    {
        return Err(ProtocolError::InvalidField);
    }
    Ok(())
}

fn is_absolute_native_path(path: &str) -> bool {
    let path = Path::new(path);
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
}

fn validate_capabilities(capabilities: &WorkerCapabilities) -> Result<(), ProtocolError> {
    validate_identity(&capabilities.runtime_id)?;
    if !capabilities.generation
        || capabilities.max_outputs == 0
        || capabilities.max_outputs > MAX_OUTPUTS
        || capabilities.max_pixels == 0
        || capabilities.max_pixels > MAX_IMAGE_PIXELS
    {
        return Err(ProtocolError::InvalidField);
    }
    Ok(())
}

fn validate_result(operation: WorkerOperation, result: &WorkerResult) -> Result<(), ProtocolError> {
    match result {
        WorkerResult::Completed { outputs } => match operation {
            WorkerOperation::Load if outputs.is_empty() => Ok(()),
            WorkerOperation::Generate
                if !outputs.is_empty() && outputs.len() <= MAX_OUTPUTS as usize =>
            {
                outputs.iter().try_for_each(validate_output)
            }
            _ => Err(ProtocolError::InvalidField),
        },
        WorkerResult::Cancelled {} => Ok(()),
        WorkerResult::Failed { message, .. } => match message {
            Some(message) if !is_safe_error_message(message) => {
                Err(ProtocolError::UnsafeErrorDetail)
            }
            _ => Ok(()),
        },
    }
}

fn validate_output(output: &WorkerOutput) -> Result<(), ProtocolError> {
    let safe_name = !output.output_name.is_empty()
        && output.output_name.len() <= MAX_IDENTITY_BYTES
        && output.output_name.ends_with(".png")
        && !output.output_name.contains(['/', '\\'])
        && !matches!(output.output_name.as_str(), "." | "..");
    if !safe_name {
        return Err(ProtocolError::InvalidField);
    }
    validate_image_bounds(output.width, output.height, 1)
}

fn validate_image_bounds(width: u32, height: u32, count: u8) -> Result<(), ProtocolError> {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if width == 0
        || height == 0
        || width > MAX_IMAGE_AXIS
        || height > MAX_IMAGE_AXIS
        || pixels > MAX_IMAGE_PIXELS
        || count == 0
        || count > MAX_OUTPUTS
    {
        return Err(ProtocolError::InvalidField);
    }
    Ok(())
}

fn validate_prompt(prompt: &str) -> Result<(), ProtocolError> {
    if prompt.trim().is_empty()
        || prompt.chars().count() > MAX_PROMPT_CHARACTERS
        || prompt
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(ProtocolError::InvalidField);
    }
    Ok(())
}

fn validate_request_id(request_id: &str) -> Result<(), ProtocolError> {
    if request_id.is_empty()
        || request_id.len() > MAX_REQUEST_ID_BYTES
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(ProtocolError::InvalidField);
    }
    Ok(())
}

fn validate_identity(identity: &str) -> Result<(), ProtocolError> {
    if identity.is_empty()
        || identity.len() > MAX_IDENTITY_BYTES
        || identity.chars().any(char::is_control)
    {
        return Err(ProtocolError::InvalidField);
    }
    Ok(())
}

fn is_safe_error_message(message: &str) -> bool {
    let normalized = message.to_ascii_lowercase();
    !message.is_empty()
        && message.len() <= MAX_ERROR_MESSAGE_BYTES
        && !message.chars().any(char::is_control)
        && !message.contains(['/', '\\'])
        && !normalized.contains("bearer ")
        && !normalized.contains("api_key")
        && !normalized.contains("apikey")
        && !normalized.contains("token=")
        && !normalized.contains("password")
        && !normalized.contains("secret")
        && !normalized.contains("sk-")
        && !normalized.contains("://")
}
