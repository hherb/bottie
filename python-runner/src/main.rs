use std::{env, io::Read, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use bottie_python_runner::{
    ExecutionStatus, MAX_REQUEST_BYTES, PythonExecutionRequest, PythonExecutionResult,
    PythonSandbox,
};

const INVALID_REQUEST_MESSAGE: &str = "The Python execution request was invalid.";
const INTERNAL_ERROR_MESSAGE: &str = "Python execution could not be started.";
const INTERNAL_RESULT_JSON: &str = concat!(
    "{\"status\":\"internal_error\",\"stdout\":\"\",",
    "\"stderr\":\"Python execution could not be started.\",\"durationMs\":0}"
);

fn main() {
    let result = run().unwrap_or_else(|_| {
        PythonExecutionResult::fixed_failure(ExecutionStatus::InternalError, INTERNAL_ERROR_MESSAGE)
    });
    let encoded =
        serde_json::to_string(&result).unwrap_or_else(|_| INTERNAL_RESULT_JSON.to_owned());
    println!("{encoded}");
}

fn run() -> Result<PythonExecutionResult> {
    let runtime_directory = runtime_argument()?;
    let request = match read_request() {
        Ok(request) if request.validate().is_ok() => request,
        Ok(_) | Err(_) => {
            return Ok(PythonExecutionResult::fixed_failure(
                ExecutionStatus::InvalidRequest,
                INVALID_REQUEST_MESSAGE,
            ));
        }
    };
    PythonSandbox::load(&runtime_directory)?.execute(&request)
}

fn runtime_argument() -> Result<PathBuf> {
    let mut arguments = env::args_os();
    let _executable = arguments.next();
    let flag = arguments.next().context("missing runtime flag")?;
    let directory = arguments.next().context("missing runtime directory")?;
    if flag != "--runtime" || arguments.next().is_some() {
        return Err(anyhow!("expected exactly --runtime <directory>"));
    }
    Ok(directory.into())
}

fn read_request() -> Result<PythonExecutionRequest> {
    let mut encoded = Vec::new();
    std::io::stdin()
        .take(MAX_REQUEST_BYTES + 1)
        .read_to_end(&mut encoded)?;
    if encoded.len() as u64 > MAX_REQUEST_BYTES {
        return Err(anyhow!("request exceeds its byte limit"));
    }
    serde_json::from_slice(&encoded).context("request is not valid JSON")
}
