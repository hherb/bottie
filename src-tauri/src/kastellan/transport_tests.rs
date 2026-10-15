//! `kastellan-cli` transport tests: prompt parsing, invocation planning, exit contract, and real child processes.

use std::path::Path;

use super::*;

fn remote(host: &str) -> KastellanConfig {
    KastellanConfig {
        host: Some(host.into()),
        cli_path: None,
    }
}

#[test]
fn extracts_the_trimmed_instruction_after_the_prefix() {
    assert_eq!(
        instruction_from_prompt("  @kastellan:  summarise my inbox ").unwrap(),
        "summarise my inbox"
    );
}

#[test]
fn rejects_prompts_the_cli_cannot_accept() {
    for prompt in [
        "@kastellan:",
        "@kastellan:   ",
        "@kastellan: --long x",
        "hello",
        "@kastellan: a\0b",
    ] {
        assert!(
            instruction_from_prompt(prompt).is_err(),
            "{prompt:?} should be rejected"
        );
    }
}

#[test]
fn bounds_the_task_at_the_composer_limit() {
    let at_limit = format!("@kastellan: {}", "x".repeat(MAX_INSTRUCTION_BYTES));
    assert!(instruction_from_prompt(&at_limit).is_ok());
    let over = format!("@kastellan: {}", "x".repeat(MAX_INSTRUCTION_BYTES + 1));
    assert!(instruction_from_prompt(&over).is_err());
}

#[tokio::test]
async fn an_early_exit_reports_the_child_error_not_a_broken_pipe() {
    // ssh failing authentication exits before reading stdin; its stderr must survive the write error.
    let plan = Invocation {
        program: "/bin/sh".into(),
        args: vec![
            "-c".into(),
            "echo 'Permission denied (publickey).' >&2; exit 255".into(),
        ],
        stdin: Some("x".repeat(1024 * 1024)),
    };
    let output = run_invocation(&plan, Duration::from_secs(10))
        .await
        .unwrap();
    let TaskOutcome::Failed { text, code } =
        interpret_output(output.status.code(), true, &output.stdout, &output.stderr)
    else {
        panic!("an ssh failure is a failure")
    };
    assert_eq!(code, "unavailable");
    assert!(text.contains("Permission denied"), "{text}");
}

#[test]
fn loopback_hosts_run_the_cli_directly_with_the_instruction_as_argv() {
    let plan = plan_invocation(
        &KastellanConfig::default(),
        Action::Ask("do it"),
        Some(Path::new("/home/u")),
    )
    .unwrap();
    assert_eq!(plan.program, "/home/u/.local/bin/kastellan-cli");
    assert_eq!(plan.args, ["ask", "do it"]);
    assert_eq!(plan.stdin, None);
    let configured = KastellanConfig {
        host: Some("127.0.0.1".into()),
        cli_path: Some("/opt/k/kastellan-cli".into()),
    };
    assert_eq!(
        plan_invocation(&configured, Action::Ask("x"), None)
            .unwrap()
            .program,
        "/opt/k/kastellan-cli"
    );
}

#[test]
fn a_local_default_needs_a_home_folder() {
    assert!(plan_invocation(&KastellanConfig::default(), Action::Probe, None).is_err());
}

#[test]
fn remote_hosts_use_ssh_with_a_fixed_command_and_the_instruction_on_stdin() {
    let instruction = "it's $(rm -rf ~); `x` \"quoted\"";
    let plan = plan_invocation(&remote("dgx"), Action::Ask(instruction), None).unwrap();
    assert_eq!(plan.program, "ssh");
    assert_eq!(plan.args[plan.args.len() - 3..][..2], ["--", "dgx"]);
    assert_eq!(
        plan.args.last().unwrap(),
        "exec \"$HOME/.local/bin/kastellan-cli\" ask \"$(cat)\""
    );
    assert!(
        plan.args.iter().all(|arg| !arg.contains("rm -rf")),
        "instruction must not reach the shell line"
    );
    assert_eq!(plan.stdin.as_deref(), Some(instruction));
}

#[test]
fn the_probe_reads_the_inbox_and_sends_nothing() {
    let local = plan_invocation(
        &KastellanConfig::default(),
        Action::Probe,
        Some(Path::new("/home/u")),
    )
    .unwrap();
    assert_eq!(local.args, ["inbox", "list"]);
    assert_eq!(local.stdin, None);
    let ssh = plan_invocation(&remote("dgx"), Action::Probe, None).unwrap();
    assert_eq!(
        ssh.args.last().unwrap(),
        "exec \"$HOME/.local/bin/kastellan-cli\" inbox list"
    );
    assert_eq!(ssh.stdin, None);
}

#[test]
fn planning_refuses_a_config_that_fails_validation() {
    for host in ["-oProxyCommand=evil", "dgx host", "dg\nx"] {
        assert!(
            plan_invocation(&remote(host), Action::Probe, None).is_err(),
            "{host:?} should be rejected"
        );
    }
    let relative = KastellanConfig {
        host: None,
        cli_path: Some("kastellan-cli".into()),
    };
    assert!(plan_invocation(&relative, Action::Probe, None).is_err());
}

#[test]
fn interprets_the_ask_exit_contract() {
    assert_eq!(
        interpret_output(Some(0), false, b"  the answer\n", b"ask: submitted task 7"),
        TaskOutcome::Reply("the answer".into())
    );
    let parked = interpret_output(
        Some(0),
        false,
        b"",
        b"ask: submitted task 7 (lane=fast); waiting\n\
          ask: task 7 is waiting for an operator decision. Run `kastellan-cli inbox list`",
    );
    let TaskOutcome::Reply(text) = parked else {
        panic!("a parked task is not a failure")
    };
    assert!(
        text.contains("operator decision") && text.contains("task 7 is waiting"),
        "{text}"
    );
    assert!(matches!(
        interpret_output(Some(255), true, b"", b"ssh: connect: refused"),
        TaskOutcome::Failed {
            code: "unavailable",
            ..
        }
    ));
    let TaskOutcome::Failed { text, code } =
        interpret_output(Some(1), false, b"", b"ask: task ended in state 'failed'")
    else {
        panic!("a non-zero exit is a failure")
    };
    assert_eq!(code, "server");
    assert!(text.contains("ended in state 'failed'"));
    // 255 from the local CLI is not an ssh failure.
    assert!(matches!(
        interpret_output(Some(255), false, b"", b""),
        TaskOutcome::Failed { code: "server", .. }
    ));
}

#[test]
fn interprets_the_probe_without_showing_inbox_contents() {
    assert_eq!(
        interpret_probe(Some(0), false, "localhost", b"").unwrap(),
        "Reached Kastellan on this computer."
    );
    assert_eq!(
        interpret_probe(Some(0), true, "dgx", b"").unwrap(),
        "Reached Kastellan on dgx over ssh."
    );
    let ssh = interpret_probe(
        Some(255),
        true,
        "dgx",
        b"ssh: Could not resolve hostname dgx",
    )
    .unwrap_err();
    assert!(
        ssh.message.contains("Could not connect to dgx over ssh")
            && ssh.message.contains("resolve")
    );
    let cli = interpret_probe(Some(1), true, "dgx", b"inbox: db connect failed").unwrap_err();
    assert!(
        cli.message.contains("could not reach Kastellan")
            && cli.message.contains("db connect failed")
    );
}

#[test]
fn reads_the_task_id_from_the_submission_line() {
    assert_eq!(
        submitted_task_id(b"warn\nask: submitted task 4821 (lane=fast); waiting for completion\n")
            .as_deref(),
        Some("4821")
    );
    assert_eq!(
        submitted_task_id(b"ask: submitted task abc (lane=fast)"),
        None
    );
}

#[test]
fn bounds_stderr_on_a_character_boundary() {
    let long = "é".repeat(STDERR_TAIL_BYTES);
    let tail = stderr_tail(&long);
    assert!(tail.starts_with('…') && tail.len() <= STDERR_TAIL_BYTES + '…'.len_utf8());
}

#[test]
fn describes_timeouts_in_natural_units() {
    assert_eq!(
        describe_duration(Duration::from_secs(30 * 60)),
        "30 minutes"
    );
    assert_eq!(describe_duration(Duration::from_secs(60)), "1 minute");
    assert_eq!(describe_duration(Duration::from_secs(20)), "20 seconds");
    assert_eq!(describe_duration(Duration::from_secs(90)), "90 seconds");
}

#[cfg(unix)]
mod process {
    use std::{os::unix::fs::PermissionsExt, time::Instant};

    use super::*;

    /// Writes an executable stand-in for `kastellan-cli` that honours its output contract.
    fn fake_cli(body: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("bottie-kastellan-cli-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("kastellan-cli");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn local(cli: &Path) -> KastellanConfig {
        KastellanConfig {
            host: None,
            cli_path: Some(cli.to_string_lossy().into_owned()),
        }
    }

    #[tokio::test]
    async fn a_local_task_returns_the_cli_reply_and_task_id() {
        let cli = fake_cli(
            r#"echo "ask: submitted task 42 (lane=fast); waiting" >&2; printf 'did: %s' "$2""#,
        );
        let plan = plan_invocation(&local(&cli), Action::Ask("it's \"$(quoted)\""), None).unwrap();
        let (outcome, task_id) = execute(&plan, Duration::from_secs(10)).await;
        assert_eq!(
            outcome,
            TaskOutcome::Reply("did: it's \"$(quoted)\"".into())
        );
        assert_eq!(task_id.as_deref(), Some("42"));
    }

    #[tokio::test]
    async fn the_probe_runs_the_inbox_subcommand() {
        let cli = fake_cli(r#"[ "$1 $2" = "inbox list" ] || exit 3; echo "secret question""#);
        let plan = plan_invocation(&local(&cli), Action::Probe, None).unwrap();
        let output = run_invocation(&plan, Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
    }

    #[tokio::test]
    async fn stdin_reaches_the_child_and_is_closed() {
        let plan = Invocation {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "cat".into()],
            stdin: Some("from stdin".into()),
        };
        let (outcome, _) = execute(&plan, Duration::from_secs(10)).await;
        assert_eq!(outcome, TaskOutcome::Reply("from stdin".into()));
    }

    #[tokio::test]
    async fn a_missing_cli_and_a_hung_cli_both_fail_with_a_reason() {
        let missing = Invocation {
            program: "/nonexistent/kastellan-cli".into(),
            args: vec![],
            stdin: None,
        };
        assert!(matches!(
            execute(&missing, Duration::from_secs(5)).await.0,
            TaskOutcome::Failed {
                code: "unavailable",
                ..
            }
        ));
        let hung = Invocation {
            program: fake_cli("sleep 30").to_string_lossy().into_owned(),
            args: vec![],
            stdin: None,
        };
        let started = Instant::now();
        let (outcome, _) = execute(&hung, Duration::from_millis(200)).await;
        assert!(
            matches!(
                outcome,
                TaskOutcome::Failed {
                    code: "timeout",
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the timeout must not wait for the child"
        );
    }

    #[tokio::test]
    async fn a_child_that_never_reads_a_large_task_still_times_out() {
        // Larger than any pipe buffer: the write cannot complete unless the child reads.
        let plan = Invocation {
            program: fake_cli("sleep 30").to_string_lossy().into_owned(),
            args: vec![],
            stdin: Some("x".repeat(1024 * 1024)),
        };
        let started = Instant::now();
        let (outcome, _) = execute(&plan, Duration::from_millis(300)).await;
        assert!(
            matches!(
                outcome,
                TaskOutcome::Failed {
                    code: "timeout",
                    ..
                }
            ),
            "{outcome:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "a blocked stdin write must be inside the timeout"
        );
    }

    /// Opt-in round trip through a real Kastellan daemon on this host, or on `BOTTIE_KASTELLAN_HOST` over ssh.
    #[tokio::test]
    #[ignore = "needs a running Kastellan; run with --ignored"]
    async fn a_real_kastellan_answers_a_probe_and_a_task() {
        let config = KastellanConfig {
            host: std::env::var("BOTTIE_KASTELLAN_HOST").ok(),
            cli_path: None,
        };
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let probe = plan_invocation(&config, Action::Probe, home.as_deref()).unwrap();
        let output = run_invocation(&probe, Duration::from_secs(60))
            .await
            .unwrap();
        let reached = interpret_probe(
            output.status.code(),
            !config.is_local(),
            config.host(),
            &output.stderr,
        );
        eprintln!("probe on {}: {reached:?}", config.host());
        reached.unwrap();

        let ask = plan_invocation(
            &config,
            Action::Ask("Reply with exactly one word: pong"),
            home.as_deref(),
        )
        .unwrap();
        let (outcome, task_id) = execute(&ask, Duration::from_secs(600)).await;
        eprintln!("task {task_id:?} on {}: {outcome:?}", config.host());
        assert!(
            task_id.is_some(),
            "the CLI should report the submitted task"
        );
        let TaskOutcome::Reply(text) = outcome else {
            panic!("{outcome:?}")
        };
        assert!(text.to_lowercase().contains("pong"), "{text}");
    }
}
