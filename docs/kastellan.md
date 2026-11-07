# Kastellan agent requests

Bottie can hand a task to a [Kastellan](https://github.com/hherb/kastellan) agent daemon and show the reply in the
conversation that asked for it. Kastellan is a separate, security-first personal agent. It runs tasks with its own
sandboxed tools and may take minutes to answer, so Bottie does not wait in the composer.

## Using it

Start a message with `@kastellan:`:

```text
@kastellan: summarise the unread messages from this morning
```

- The selected model is not used, so no model needs to be loaded or reachable.
- A waiting response appears immediately, naming the host the task went to.
- While the task runs, the waiting response shows Kastellan's progress as it happens: its plan, each step done or
  failed, and corrections. Progress is not saved; only the reply is.
- **Stop** cancels the task. The response is saved as stopped once Kastellan confirms, which can take up to one
  model turn.
- **Correct** sends a correction ("use the other account") that Kastellan's planner reads on its next turn. Kastellan
  takes corrections only in its notebook planner mode.
- The composer stays free. You can keep chatting in **other** conversations while Kastellan works.
- The reply replaces the waiting response when it arrives. Its footer shows the Kastellan task number, host, and
  duration, for example `Kastellan task 204 on dgx · 34s`.
- **Regenerate** and **Edit and resend** on a Kastellan request send it to Kastellan again, not to the model.
- A task is limited to 32 KiB of text, the same limit as the composer draft.
- If the task cannot be sent at all, the reason becomes the saved response. That covers invalid settings, an
  unreachable host, or a Kastellan too old to have `kastellan-cli session`. Fix the cause, then Regenerate.

The conversation that is waiting is busy until Kastellan answers. Bottie keeps at most one running response per
conversation, and a Kastellan task is a running response, so sending another message there is refused until the reply
lands.

## Choosing the host

Settings → **Kastellan agent**:

| Field              | Default                                  | Meaning                                                                                                        |
| ------------------ | ---------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Host               | `localhost`                              | `localhost`, `127.0.0.1`, or `::1` runs Kastellan's CLI on this computer. Anything else is an ssh destination. |
| kastellan-cli path | `~/.local/bin/kastellan-cli` on the host | Only needed when Kastellan was installed somewhere else. Must be absolute on this computer.                    |

**Test connection** runs `kastellan-cli inbox list` with the drafts, without saving them. It proves that the CLI runs and
reaches Kastellan's database. It submits no task, and the inbox contents are discarded rather than shown.

A remote host is anything `ssh` accepts: a name, an address, `user@host`, or an alias from `~/.ssh/config`. Bottie
runs `ssh` in batch mode, so authentication must already work without a prompt, through your ssh agent or a key
without a passphrase. Bottie never asks for or stores an ssh password or key.

Settings are saved as `kastellan.json` in Bottie's config folder, for example
`~/Library/Application Support/com.bottie.app/kastellan.json` on macOS:

```json
{ "host": "dgx" }
```

Prefer Settings to editing this file by hand. A hand-edited file is validated with the same rules, and an invalid one
makes requests fail with an explanation instead of falling back to another host.

## How it works

```text
Composer ──@kastellan:──▶ start_kastellan_task ──▶ provider run (provider "kastellan", model = host)
                                │                        │
                                │                        └─ waiting response saved under the request
                                └─ background task, one session per request:
                                     kastellan-cli session                    (this computer)
                                     ssh <host> 'exec kastellan-cli session'  (remote)
                                     ⇄ JSON-RPC on stdin/stdout: task.submit, task.follow,
                                       task.cancel (Stop), task.correct (Correct)
                                   → event "kastellan-task-progress" per notebook entry
                                   → reply + provenance saved, run closed
                                   → event "kastellan-task-finished" → open conversation reloads
```

- Native module: `src-tauri/src/kastellan/`. `config.rs` holds settings and validation, `transport.rs` plans
  `kastellan-cli` invocations and reads their exit status, `session.rs` is the pure session protocol,
  `session_run.rs` runs one session, `settings.rs` holds the Settings commands, and `mod.rs` holds the dispatch,
  Stop and Correct commands and persistence.
- WebView: `src/lib/kastellan.ts` holds IPC and the presentation rules, `src/routes/kastellan-state.svelte.ts` holds
  dispatch, progress, Stop/Correct and reload, `src/lib/KastellanProgress.svelte` is the live part of a running
  response, and `src/lib/KastellanSettingsControl.svelte` is the Settings section.
- The reply is a normal durable assistant message. Its provenance line is stored in the message's reasoning block, so it
  is durable but never enters model context, and presentation shows it as the footer.
- Kastellan exchanges are ordinary conversation turns. If you later ask the selected model in the same conversation,
  the `@kastellan:` request and Kastellan's reply are part of the context it sees.

### `kastellan-cli session` contract

| Kastellan says                         | Bottie shows                                                                                |
| -------------------------------------- | ------------------------------------------------------------------------------------------- |
| `notebook.entry`                       | One progress line in the waiting response.                                                  |
| `task.finished`, `completed`           | The answer.                                                                                 |
| `task.finished`, `cancelled`           | "Kastellan stopped this task.", saved as stopped.                                           |
| `task.finished`, any other state       | Kastellan's reason, as a failure.                                                           |
| `task.awaiting_operator`               | The task paused for an operator decision; answer it with `kastellan-cli inbox` on the host. |
| `task.follow_failed`, or the CLI exits | A failure with Kastellan's reason, or ssh's (exit 255) or the CLI's error, bounded to 2 KiB. |

The request is limited to 30 minutes, after which the CLI is killed. Closing a session never cancels its task, so the
task keeps running, and `kastellan-cli tasks list` on the host shows it. The session needs a Kastellan with
`kastellan-cli session` (Kastellan PR #986 and later).

## Security boundary

- **No shell sees your message.** The task and any correction travel as JSON-RPC lines on the session's stdin. The
  ssh command line is fixed: `exec <cli> session`.
- **Hosts cannot become ssh options.** A host starting with `-` or containing whitespace or control characters is
  refused, and `--` precedes the destination.
- **The CLI path is configuration, not input.** It is written by you in Settings. Remotely it is inserted into the ssh
  command line unquoted, so the remote shell can expand `~` or `$HOME`. Quote it yourself if it contains spaces. It is
  validated only for control characters and length.
- **Kastellan's own boundary is unchanged.** `kastellan-cli` talks to Kastellan's Postgres over a local socket as the
  operating-system user who installed it. Bottie gets no more access than that user already has at a terminal on that
  host, and every task still goes through Kastellan's scheduler, oversight, and audit log.
- Requests use Kastellan's operator CLI, so they are trusted like an operator typing at that host. They do not pass
  through Kastellan's messaging-channel screening, which exists for remote peers.

## Known limits

- The waiting conversation is busy until the reply arrives, as described above.
- A task still pending when Bottie quits is marked interrupted on the next start. The task keeps running in Kastellan;
  read its result with `kastellan-cli tasks status <id>` on the host.
- Kastellan's operator questions are reported as text, not answered inside Bottie.
- Progress is not saved. A conversation reopened while its task runs shows Stop and Correct at once, and its earlier
  progress returns with the next progress line.
- The answer itself arrives whole; only the progress is live.

## Tests

```sh
cargo test --manifest-path src-tauri/Cargo.toml kastellan                 # unit and fake-CLI process tests
npx vitest run src/lib/kastellan src/routes/kastellan-state src/routes/page-state-kastellan

# Opt-in live round trip against a running Kastellan (probe, then one task):
cargo test --manifest-path src-tauri/Cargo.toml a_real_kastellan -- --ignored --nocapture
BOTTIE_KASTELLAN_HOST=dgx cargo test --manifest-path src-tauri/Cargo.toml a_real_kastellan -- --ignored --nocapture
```
