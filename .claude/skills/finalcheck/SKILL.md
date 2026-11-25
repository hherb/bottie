---
name: finalcheck
description: final pre-merge pass on the current PR (or the earliest open PR when the branch has none) — review, fix everything, run the local gate, then drive GitHub CI to green
disable-model-invocation: false
allowed-tools: Bash(git *), Bash(gh *), Bash(cargo *), Bash(npm *), Bash(npx *), Bash(node *), Bash(python3 -m unittest *), Bash(actionlint *)
---
Run these steps in order on the PR for the current branch. First run
`pwd && git branch --show-current && git worktree list`. If the PR's branch is checked out in a worktree,
`cd` into it. Then find the PR with `gh pr view --json number,url,headRefOid,baseRefName`.

If the current branch has no PR, take the earliest open one instead:
`gh pr list --state open --json number,headRefName,createdAt --jq 'sort_by(.createdAt) | .[0]'`.
If there are no open PRs at all, stop and tell me. Otherwise say which PR you picked, then switch to it:
- If its branch is checked out in a worktree (`git worktree list`), `cd` into that worktree.
- Otherwise run `gh pr checkout <number>`. Do this only when `git status --porcelain` is empty. With
  uncommitted changes, stop and tell me rather than stashing or discarding them.

Then re-run `gh pr view --json number,url,headRefOid,baseRefName` to confirm you are on the PR.

**Stacked PRs.** If `baseRefName` is not `main`, the PR is stacked on another PR's branch. Run this check on the
base PR first, or tell me which one you are doing. Bring a stacked branch up to date from its base branch, not
from `main`.

The repository root is not a Cargo workspace. Run every cargo command from the repo root with
`--manifest-path src-tauri/Cargo.toml` (CONTRIBUTING.md), or from `src-tauri/`. A foreground full
`cargo test` can exceed the 600 s tool timeout, so run it with `run_in_background` and grep the log.

1. **Review.** Invoke the `pr-review-toolkit:review-pr` skill via the Skill tool and let it finish.
2. **Fix.**
   - First bring the branch up to date: `git fetch origin && git merge origin/<baseRefName>`, and resolve
     any conflicts. HANDOVER.md and ROADMAP.md conflict most often. Keep both sides' facts, and never drop
     another PR's entry.
   - Then invoke the `fixall` skill via the Skill tool on that review's findings. Per the session rules
     ("avoid technical debt"), every confirmed finding is either fixed in this PR or lodged as a GitHub
     issue with file refs and a failure scenario. None may live only in the PR body or the chat.
   - If the fixes change what HANDOVER.md or ROADMAP.md say (test counts, line counts, open PRs, next
     items), update them. Grep the docs for the old numbers.
3. **Local gate.** CI runs **no** Rust or frontend unit tests. It runs only CodeQL and packaging smokes
   (see step 4), so this step is the only place the test suites run. Judge each command by its **exit
   code**, never by filtered output: `cmd | tail` reports `tail`'s status, so use `set -o pipefail`.
   Run from the repo root:
   - `git diff --check origin/<baseRefName>...HEAD`
   - `npm run format:check`: this check already fails on `main` for some files. Fail only on files this
     PR touched, using `npx prettier --check <changed files>`.
   - `npm run check` and `npm test`
   - `npm run build` when frontend, `static/` or Svelte config changed
   - `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
   - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets`. It is not `-D warnings`: `main`
     carries pre-existing style warnings. Count the `^warning:` lines on this branch and on
     `origin/<baseRefName>` (use `git worktree add` for the base, never stash). The PR must add none.
   - `cargo test --manifest-path src-tauri/Cargo.toml`, which covers the library and every integration
     suite. Compare the library test count with HANDOVER's figure.
   - `cargo test --manifest-path python-runner/Cargo.toml --locked` when `python-runner/` changed
   - `cd local-image-worker && python3 -m unittest discover -p 'test_*.py'` when `local-image-worker/` changed
   - `cargo check --manifest-path src-tauri/Cargo.toml --all-targets --features local-image-runtime-proof`
     when `src-tauri/src/local_image_worker*` changed. It is broken on `main` (#249). Until that is
     fixed, the error count must not grow compared with the base.
   - The 500-line cap: run `wc -l` on every changed source file. A file this PR pushed over 500 lines is a
     finding. Files already over the cap are listed in HANDOVER.

   Fix failures at the root cause, using TDD where it is a code bug. Commit with a message naming what
   failed, then push.
   - Before moving on, confirm the push landed: `git fetch origin && git status -sb` must show no
     `ahead`, and the PR's `headRefOid` must equal `git rev-parse HEAD`.
4. **Drive CI to green.** Loop:
   - Watch the checks for the pushed head commit with `gh pr checks --watch --fail-fast`. Checks can take a
     minute to register, so wait if it reports none yet. The MSI smoke takes about 25 minutes, so use
     `run_in_background` or a Monitor rather than blocking.
   - What runs on a PR:
     - Always: **CodeQL** default setup (`Analyze (actions|c-cpp|javascript-typescript|python|rust)` plus the
       `CodeQL` summary). It has no workflow file. Its findings show as code-scanning alerts on the PR.
     - Path-filtered (`on.pull_request.paths` in each file under `.github/workflows/`):
       - `Locked DEB and isolated smoke` (linux-package-smoke, ubuntu-24.04)
       - `Locked MSI and isolated smoke` (windows-package-smoke, windows-2025). Both run on any `src/**`,
         `src-tauri/**`, `static/**` or package-file change.
       - `Landlock, seccomp, rlimits, and lifecycle` (linux-python-containment)
       - `Restricted runner and Job Object` (windows-python-appcontainer)
       - the python-runtime-provenance jobs, for `python-runner/**`, `scripts/*python*` and the like
     - A docs-only PR runs only CodeQL. That is green.
     - The distribution, updater-publication and Store workflows are `workflow_dispatch`/`workflow_call`
       only. Never trigger them: they use signing secrets and protected environments.
   - All green → done. Report the PR URL and the final check list. `main` has no branch protection and no
     required checks, so "all green" means every check on the head commit passed. A path-filtered job that
     **should** have run but is missing means its `paths` filter misses a file this PR changed: report it.
   - A check failed → read the cause: `gh run list --branch "$(git branch --show-current)" --limit 5`, then
     `gh run view <run-id> --log-failed`. Reproduce it locally with the command the failing step runs
     (the workflow file is the source of truth):
     - **DEB smoke:** run the "Verify Linux package contracts" commands (`npm run icons:check`, the
       `node --check scripts/linux-*.mjs` lines, `npm run package:linux:test`,
       `npm test -- scripts/linux-distribution.test.mjs`, `npm run package:linux:distribution:test:integration`).
       The smoke itself (`npm run package:linux:smoke`) is Linux-only. Reproduce it in Docker (`ubuntu:24.04`
       with the apt packages from "Install Linux desktop build dependencies"; start the daemon with
       `open -a Docker`). Don't guess.
     - **MSI smoke:** run the "Verify Windows package contracts" commands locally (`npm run icons:check`,
       `node --check scripts/windows-package*.mjs`, `npm run package:windows:test`). The MSI build itself
       cannot run on this Mac. Fix from the log, and say in the report that the fix was verified only by CI.
     - **Python containment / provenance:** `cargo test --manifest-path python-runner/Cargo.toml --locked`
       and the workflow's `npm test -- scripts/…test.mjs` lines. The native proofs are Linux- or
       Windows-only.
     - **CodeQL:** read the alert (`gh api repos/hherb/bottie/code-scanning/alerts?ref=refs/pull/<n>/head`).
       Fix the code. Never dismiss an alert without my permission.
     - **A workflow file changed:** `actionlint` (if installed; otherwise say so).
   - **A head-commit job that is CANCELLED or timed out with no log** was not superseded. The runner was
     killed or hit `timeout-minutes` (the smokes cap at 45). Treat it as a code bug, such as a hung test,
     a hung smoke launch or a runaway build, and find the cause. Don't just rerun it as a flake.
   - If the failure is not caused by the code, rerun once with `gh run rerun <run-id> --failed`. That means
     a runner or infra outage, a network flake, an apt or npm registry failure, or a failed ONNX Runtime
     mirror/cache download in `.github/actions/onnx-runtime`. If it fails the same way again, stop and
     report rather than changing code to work around it. The Actions API returns empty or stale data during
     an outage, so re-query before concluding a run never happened.
   - Never "fix" CI by skipping, `#[ignore]`-ing or deleting tests, or by adding `#[allow(...)]`. Don't
     narrow a workflow's `paths` filter either. Never change a pinned hash or digest to match new bytes:
     the ONNX Runtime archive (`.github/actions/onnx-runtime`), `runtime-assets.json`, the pinned speech or
     VAD models (`microphone/pinned_assets.rs`) or the worker bundle hashes (`model_package.rs`). Each needs
     my explicit permission. If the same failure survives three fix attempts, stop and report what you
     tried.
5. **Not covered by CI or the local gate.** If the PR touches these, say what you ran, and list the rest as
   not verified rather than claiming them:
   - **Live providers** (oMLX, Ollama, Localmail, Kastellan): the `#[ignore]`d live tests need running
     services. Their commands are in README.md.
   - **Native UI flows:** CONTRIBUTING.md asks for a manual run of provider, credential, persistence and
     cancellation changes in the Tauri app. Use the `run` skill if available, or list it as owed.
   - **Real audio devices** (microphone, Silero gate, voice conversation mode, Bluetooth input).
   - **Local image hardware** (MOSS MLX on the Mac, CUDA and the Linux clean runtime on the DGX, the
     managed Qwen-Image worker bundle): rebuild and re-pin steps are in HANDOVER's "Owed hardware
     re-checks".
   - **Windows and Linux native behaviour** beyond what the smokes exercise.
