<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";

  import Icon from "$lib/Icon.svelte";
  import { providerErrorFromUnknown } from "$lib/inference";
  import {
    KASTELLAN_DEFAULT_HOST,
    KASTELLAN_PREFIX,
    getKastellanSettings,
    testKastellanConnection,
    updateKastellanSettings,
    type KastellanSettings,
  } from "$lib/kastellan";
  import { kastellanDraftChanged, kastellanRouteLabel } from "$lib/kastellan-settings";

  type Props = { disabled: boolean };

  let { disabled }: Props = $props();
  let initialized = false;
  let saved = $state<KastellanSettings | null>(null);
  let host = $state("");
  let cliPath = $state("");
  let busy = $state<"loading" | "saving" | "testing" | null>(null);
  let feedback = $state("");
  let failed = $state(false);

  $effect(() => {
    if (!initialized) {
      initialized = true;
      void loadSettings();
    }
  });

  /** Copies saved settings into the editable drafts. */
  function applySettings(settings: KastellanSettings): void {
    saved = settings;
    host = settings.host === KASTELLAN_DEFAULT_HOST ? "" : settings.host;
    cliPath = settings.cliPath ?? "";
  }

  /** Loads the effective host and CLI path. */
  async function loadSettings(): Promise<void> {
    if (!isTauri()) return;
    busy = "loading";
    try {
      applySettings(await getKastellanSettings());
    } catch (error) {
      showError(error);
    } finally {
      busy = null;
    }
  }

  /** Clears stale feedback as soon as a draft changes. */
  function edit(update: () => void): void {
    update();
    feedback = "";
    failed = false;
  }

  /** Checks the drafts by reading the Kastellan operator inbox; no task is submitted and nothing is saved. */
  async function testConnection(): Promise<void> {
    if (!isTauri() || disabled || busy) return;
    busy = "testing";
    feedback = "";
    failed = false;
    try {
      feedback = await testKastellanConnection(host, cliPath);
    } catch (error) {
      showError(error);
    } finally {
      busy = null;
    }
  }

  /** Saves the drafts on their own. */
  async function saveSettings(): Promise<void> {
    if (!isTauri() || disabled || busy) return;
    try {
      await persist();
      feedback = `Saved. ${KASTELLAN_PREFIX} requests go to ${kastellanRouteLabel(saved!)}.`;
    } catch (error) {
      showError(error);
    }
  }

  /** Includes unsaved drafts in the dialog's main Save. */
  export async function savePendingChanges(): Promise<void> {
    if (!isTauri()) return;
    if (busy) throw new Error("Wait for the Kastellan check to finish before saving Settings.");
    if (!kastellanDraftChanged(saved, host, cliPath)) return;
    await persist();
  }

  /** Writes validated drafts natively and adopts the normalized result. */
  async function persist(): Promise<void> {
    busy = "saving";
    feedback = "";
    failed = false;
    try {
      applySettings(await updateKastellanSettings(host, cliPath));
    } finally {
      busy = null;
    }
  }

  /** Converts one native failure into the shared provider-error presentation. */
  function showError(error: unknown): void {
    feedback = providerErrorFromUnknown(error).message;
    failed = true;
  }
</script>

<section class="provider-setting kastellan-setting" aria-labelledby="kastellan-settings-title">
  <div class="provider-setting-heading">
    <span>
      <strong id="kastellan-settings-title">Kastellan agent</strong>
      <small>Messages starting with <code>{KASTELLAN_PREFIX}</code> go to Kastellan instead of the model</small>
    </span>
    <span class="local-badge"><Icon name="shield" size={12} /> {saved?.remote ? "ssh" : "This computer"}</span>
  </div>

  <label for="kastellan-host">Host</label>
  <input
    id="kastellan-host"
    type="text"
    value={host}
    placeholder={KASTELLAN_DEFAULT_HOST}
    oninput={(event) => edit(() => (host = event.currentTarget.value))}
    disabled={!isTauri() || disabled || busy !== null}
    autocomplete="off"
    autocapitalize="off"
    spellcheck="false"
  />

  <label for="kastellan-cli-path">kastellan-cli path <small>optional</small></label>
  <input
    id="kastellan-cli-path"
    type="text"
    value={cliPath}
    placeholder="~/.local/bin/kastellan-cli on the host"
    oninput={(event) => edit(() => (cliPath = event.currentTarget.value))}
    disabled={!isTauri() || disabled || busy !== null}
    autocomplete="off"
    autocapitalize="off"
    spellcheck="false"
  />

  <p class="kastellan-boundary">
    Leave Host empty for Kastellan on this computer. Any other host — a name, address, or alias from
    <code>~/.ssh/config</code> — is reached with <code>ssh</code> using your existing keys; Bottie never asks for or
    stores a password. Your message is sent to <code>kastellan-cli ask</code> as data, never as part of a shell command.
  </p>
  <p class="kastellan-boundary">
    Test connection only reads the Kastellan operator inbox, without submitting a task or showing its contents.
  </p>

  <div class="kastellan-actions">
    <button type="button" disabled={!isTauri() || disabled || busy !== null} onclick={testConnection}>
      {busy === "testing" ? "Testing…" : "Test connection"}
    </button>
    <button
      type="button"
      class="primary"
      disabled={!isTauri() || disabled || busy !== null || !kastellanDraftChanged(saved, host, cliPath)}
      onclick={saveSettings}>{busy === "saving" ? "Saving…" : "Save Kastellan host"}</button
    >
  </div>

  {#if feedback}
    <p class:error={failed} class:success={!failed} class="test-result" role={failed ? "alert" : "status"}>
      {feedback}
    </p>
  {/if}
</section>
