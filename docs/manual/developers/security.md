# Security and privacy

[Back to the manual index](index.md)

The WebView is untrusted; Rust enforces policy. Providers, websites, email, files, models, and child workers are also
untrusted inputs.

## Invariants

1. Secrets remain in the OS vault/native process; UI receives status only.
2. Paths, handles, hardware/platform identities, and private bytes remain native. Use safe labels and opaque tokens.
3. IPC grants product operations, never generic HTTP/filesystem/shell/SQL/updater/audio authority.
4. Local routing is loopback-only; cloud origins and optional context/tool routes are explicit.
5. External inputs have byte/character/item/nesting/time/redirect/round/aggregate bounds before costly collection.
6. Audit is durable but sanitized and bounded.
7. Invalid state, missing capability, uncertain destination, failed verification, or races fail closed.

## IPC threat model

Assume malicious JavaScript invokes every registered command with arbitrary JSON, in any order. TypeScript and disabled
buttons are not security. Rust validates the current native state and identity each time.

Ask for each DTO:

- Can unknown fields smuggle a credential or alternate destination?
- Can an ID target another conversation/run/session?
- Can stale completion mutate a replacement session?
- Can errors reveal paths, headers, credentials, database details, or native IDs?
- Are byte limits used where protocols care about UTF-8 bytes?
- Can per-item limits multiply into excessive aggregate work?

Add adversarial native contract tests.

## Network, files, and rendering

Dedicated native clients enforce scheme, host/port, DNS/IP classes, redirects, proxies, TLS/pinning, deadlines, size,
and parsing policy. Re-validate SSRF policy at redirects/resolution, not only the initial URL. Never put secrets in URLs
or diagnostics. Test loopback/private/link-local/credential-bearing/malformed destinations where relevant.

Extensions are untrusted. Detect/validate native content, bound it, copy to app-owned storage, and normalize images.
Treat Markdown, documents, email, search results, provider output, and tool JSON as hostile. Preserve Markdown
sanitization, custom preview protocols, and CSP; never introduce raw HTML for convenience.

Microphone/transcript, speech, updater, Python, and image-worker/model state have dedicated controllers. Separate user
actions are separate grants: recording audio does not authorize sending or retaining it. Downloads/workers require
pinned identity/integrity, bounded versioned protocols, staged promotion, and deterministic cleanup.

## Review template

1. What data/authority enters each process, durable store, network, or helper?
2. Which exact action/setting authorizes it?
3. What are input, time, memory, output, and concurrency bounds?
4. Where are secrets, paths, bytes, IDs logged/exported/backed up/deleted?
5. What happens on cancellation, restart, corruption, partial write, stale completion, and missing capability?
6. Which adversarial tests prove the boundary?
7. Which platforms were actually exercised?

Review Tauri capabilities, configuration, CSP, custom protocols, and policy tests when adding plugins, protocols, events,
or native permissions. Permission expansion needs explicit threat-model justification.
