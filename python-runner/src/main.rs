#[cfg(target_os = "linux")]
use std::io::Write;
use std::{env, io::Read, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use bottie_python_runner::{
    ExecutionStatus, MAX_REQUEST_BYTES, PythonExecutionRequest, PythonExecutionResult,
    PythonSandbox,
};

const INVALID_REQUEST_MESSAGE: &str = "The Python execution request was invalid.";
const INTERNAL_RESULT_JSON: &str = concat!(
    "{\"status\":\"internal_error\",\"stdout\":\"\",",
    "\"stderr\":\"Python execution could not be started.\",\"durationMs\":0}"
);

fn main() {
    let output = run().unwrap_or_else(|_| RunnerOutput {
        encoded: INTERNAL_RESULT_JSON.to_owned(),
        #[cfg(target_os = "linux")]
        hold_for_parent_close: false,
    });
    println!("{}", output.encoded);
    #[cfg(target_os = "linux")]
    if output.hold_for_parent_close {
        let _ = std::io::stdout().flush();
        loop {
            std::thread::park();
        }
    }
}

struct RunnerOutput {
    encoded: String,
    #[cfg(target_os = "linux")]
    hold_for_parent_close: bool,
}

fn run() -> Result<RunnerOutput> {
    let arguments = runner_arguments()?;
    let request = match read_request() {
        Ok(request) if request.validate().is_ok() => request,
        Ok(_) | Err(_) => {
            return encode_result(
                PythonExecutionResult::fixed_failure(
                    ExecutionStatus::InvalidRequest,
                    INVALID_REQUEST_MESSAGE,
                ),
                false,
            );
        }
    };
    let sandbox = PythonSandbox::load(&arguments.runtime_directory)?;
    #[cfg(target_os = "linux")]
    if arguments.linux_contained {
        let execution =
            sandbox.execute_linux_contained(&request, arguments.denied_fixture.as_deref())?;
        if arguments.denied_fixture.is_some() {
            if execution.result.status != ExecutionStatus::Ok
                || execution.result.stdout.trim() != "42"
            {
                return Err(anyhow!("the containment probe execution failed"));
            }
            return Ok(RunnerOutput {
                encoded: serde_json::to_string(&execution.evidence)
                    .context("could not encode containment evidence")?,
                hold_for_parent_close: false,
            });
        }
        return encode_result(execution.result, arguments.hold_for_parent_close);
    }
    encode_result(sandbox.execute(&request)?, false)
}

fn encode_result(
    result: PythonExecutionResult,
    _hold_for_parent_close: bool,
) -> Result<RunnerOutput> {
    Ok(RunnerOutput {
        encoded: serde_json::to_string(&result).context("could not encode the execution result")?,
        #[cfg(target_os = "linux")]
        hold_for_parent_close: _hold_for_parent_close,
    })
}

struct RunnerArguments {
    runtime_directory: PathBuf,
    #[cfg(target_os = "linux")]
    linux_contained: bool,
    #[cfg(target_os = "linux")]
    denied_fixture: Option<PathBuf>,
    #[cfg(target_os = "linux")]
    hold_for_parent_close: bool,
}

fn runner_arguments() -> Result<RunnerArguments> {
    let mut arguments = env::args_os();
    let _executable = arguments.next();
    let first_flag = arguments.next().context("missing runtime flag")?;
    #[cfg(target_os = "linux")]
    let (flag, linux_contained) = if first_flag == "--linux-contained" {
        (arguments.next().context("missing runtime flag")?, true)
    } else {
        (first_flag, false)
    };
    #[cfg(not(target_os = "linux"))]
    let flag = first_flag;
    let directory = arguments.next().context("missing runtime directory")?;
    #[cfg(target_os = "linux")]
    let (denied_fixture, hold_for_parent_close) = match arguments.next() {
        Some(probe) if linux_contained && probe == "--linux-containment-probe" => (
            Some(arguments.next().context("missing denied fixture")?.into()),
            false,
        ),
        Some(probe) if linux_contained && probe == "--linux-parent-close-proof" => (None, true),
        Some(_) => return Err(anyhow!("unexpected runner argument")),
        None => (None, false),
    };
    if flag != "--runtime" || arguments.next().is_some() {
        return Err(anyhow!("expected exactly --runtime <directory>"));
    }
    Ok(RunnerArguments {
        runtime_directory: directory.into(),
        #[cfg(target_os = "linux")]
        linux_contained,
        #[cfg(target_os = "linux")]
        denied_fixture,
        #[cfg(target_os = "linux")]
        hold_for_parent_close,
    })
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
