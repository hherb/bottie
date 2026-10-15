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
- The composer stays free. You can keep chatting in **other** conversations while Kastellan works.
- The reply replaces the waiting response when it arrives. Its footer shows the Kastellan task number, host, and
  duration, for example `Kastellan task 204 on dgx · 34s`.
- **Regenerate** and **Edit and resend** on a Kastellan request send it to Kastellan again, not to the model.
- A task is limited to 32 KiB of text, the same limit as the composer draft.
- If the task cannot be sent at all, the reason becomes the saved response. That covers invalid settings, a task
  starting with `-`, or an unreachable host. Fix the cause, then Regenerate.

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
                                └─ background task:
                                     kastellan-cli ask ...        (this computer)
                                     ssh <host> 'exec kastellan-cli ask "$(cat)"'   (remote; task on stdin)
                                   → reply + provenance saved, run closed
                                   → event "kastellan-task-finished" → open conversation reloads
```

- Native module: `src-tauri/src/kastellan/`. `config.rs` holds settings and validation, `transport.rs` plans and
  interprets `kastellan-cli` invocations, `settings.rs` holds the Settings commands, and `mod.rs` holds the dispatch
  command and persistence.
- WebView: `src/lib/kastellan.ts` holds IPC and the presentation rules, `src/routes/kastellan-state.svelte.ts` holds
  dispatch and reload, and `src/lib/KastellanSettingsControl.svelte` is the Settings section.
- The reply is a normal durable assistant message. Its provenance line is stored in the message's reasoning block, so it
  is durable but never enters model context, and presentation shows it as the footer.
- Kastellan exchanges are ordinary conversation turns. If you later ask the selected model in the same conversation,
  the `@kastellan:` request and Kastellan's reply are part of the context it sees.

### `kastellan-cli ask` contract

| Exit  | stdout     | Bottie shows                                                                                          |
| ----- | ---------- | ----------------------------------------------------------------------------------------------------- |
| 0     | reply text | The reply.                                                                                            |
| 0     | empty      | Kastellan paused the task for an operator decision; answer it with `kastellan-cli inbox` on the host. |
| 255   | (ssh)      | Could not reach the host over ssh, with ssh's own error.                                              |
| other | —          | Kastellan could not complete the task, with the CLI's error, bounded to 2 KiB.                        |

The CLI is killed after 30 minutes. The Kastellan task itself keeps running, and `kastellan-cli tasks list` on the host
shows it.

## Security boundary

- **No shell sees your message.** Locally it is a single argv element of `kastellan-cli ask`. Remotely the ssh command
  line is fixed, `exec <cli> ask "$(cat)"`, and the task travels on stdin.
- **Hosts cannot become ssh options.** A host starting with `-` or containing whitespace or control characters is
  refused, and `--` precedes the destination.
- **The CLI path is configuration, not input.** It is written by you in Settings. Remotely it is inserted into the ssh
  command line unquoted, so the remote shell can expand `~` or `$HOME`. Quote it yourself if it contains spaces. It is
  validated only for control characters and length.
- **The remote login shell must be POSIX-compatible**, such as sh, bash, or zsh, because the fixed command uses
  `"$(cat)"`. csh and tcsh logins are not supported.
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
- There is no cancel control. Use `kastellan-cli tasks cancel <id>` on the host.
- Replies are not streamed; Kastellan answers each task once.

## Tests

```sh
cargo test --manifest-path src-tauri/Cargo.toml kastellan                 # unit and fake-CLI process tests
npx vitest run src/lib/kastellan src/routes/kastellan-state src/routes/page-state-kastellan

# Opt-in live round trip against a running Kastellan (probe, then one task):
cargo test --manifest-path src-tauri/Cargo.toml a_real_kastellan -- --ignored --nocapture
BOTTIE_KASTELLAN_HOST=dgx cargo test --manifest-path src-tauri/Cargo.toml a_real_kastellan -- --ignored --nocapture
```
