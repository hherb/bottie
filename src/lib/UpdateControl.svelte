<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";

  import {
    cancelUpdateOperation,
    checkForUpdate,
    installUpdate,
    updateErrorFromUnknown,
    type UpdateCheckResult,
  } from "$lib/updater";

  let status = $state<"idle" | "checking" | "available" | "installing" | "installed" | "noUpdate" | "error">("idle");
  let result = $state<UpdateCheckResult | null>(null);
  let message = $state("");

  /** Runs one user-requested native check without downloading an update. */
  async function check(): Promise<void> {
    if (!isTauri() || status === "checking" || status === "installing") return;
    status = "checking";
    message = "Checking the signed release manifest…";
    result = null;
    try {
      result = await checkForUpdate();
      if (result.status === "updateAvailable") {
        status = "available";
        message = `Bottie ${result.version} is ready to review.`;
      } else {
        status = "noUpdate";
        message = `Bottie ${result.currentVersion} is up to date.`;
      }
    } catch (error) {
      status = "error";
      message = updateErrorFromUnknown(error).message;
    }
  }

  /** Installs only the exact candidate returned by the preceding check. */
  async function install(): Promise<void> {
    if (!isTauri() || status !== "available") return;
    status = "installing";
    message = "Downloading and verifying the reviewed update…";
    try {
      const installed = await installUpdate();
      status = "installed";
      message = `Bottie ${installed.version} is installed. Restart Bottie to use it.`;
    } catch (error) {
      status = "error";
      message = updateErrorFromUnknown(error).message;
    }
  }

  /** Requests cancellation without exposing an updater operation identity. */
  async function cancel(): Promise<void> {
    const requested = await cancelUpdateOperation().catch(() => false);
    if (requested) message = "Cancelling the update action…";
  }
</script>

<section class="update-control" aria-labelledby="application-updates-title">
  <div class="update-heading">
    <span>
      <strong id="application-updates-title">Application updates</strong>
      <small>Bottie checks only its fixed HTTPS GitHub release manifest.</small>
    </span>
    <button type="button" disabled={!isTauri() || status === "checking" || status === "installing"} onclick={check}>
      {status === "checking" ? "Checking…" : "Check for updates"}
    </button>
  </div>
  <p>Nothing downloads or installs until you choose it. Update verification and installation stay in native Rust.</p>
  {#if status === "available" && result?.version}
    <div class="update-candidate">
      <span
        ><strong>Version {result.version}</strong><small>{result.notes ?? "Release notes are unavailable."}</small
        ></span
      >
      <button type="button" class="primary" onclick={install}>Install update</button>
    </div>
  {/if}
  {#if status === "checking" || status === "installing"}
    <button type="button" class="cancel-update" onclick={cancel}>Cancel update action</button>
  {/if}
  {#if message}
    <p class:error={status === "error"} role={status === "error" ? "alert" : "status"}>{message}</p>
  {/if}
</section>

<style>
  .update-control {
    display: grid;
    gap: 0.72rem;
    padding: 1rem;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.018);
  }

  .update-heading,
  .update-candidate {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }

  span {
    display: grid;
    gap: 0.18rem;
  }

  strong {
    color: var(--text);
    font-size: 11px;
    font-weight: 620;
  }

  small,
  p {
    color: var(--subtle-text);
  }

  small {
    font-size: 8px;
  }

  p {
    margin: 0;
    font-size: 9px;
    line-height: 1.45;
  }

  button {
    padding: 8px 11px;
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.035);
    color: var(--muted-strong);
    cursor: pointer;
    font-size: 9px;
    white-space: nowrap;
  }

  button:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }

  button.primary {
    border-color: transparent;
    background: linear-gradient(135deg, #765ed8, #8c74e8);
    color: white;
  }

  .update-candidate {
    padding-top: 0.72rem;
    border-top: 1px solid var(--line);
  }

  .cancel-update {
    justify-self: start;
  }

  .error {
    color: #d89a91;
  }

  @media (max-width: 760px) {
    .update-heading,
    .update-candidate {
      align-items: stretch;
      flex-direction: column;
    }
  }
</style>
