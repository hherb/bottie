<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";

  import Icon from "$lib/Icon.svelte";
  import { providerErrorFromUnknown } from "$lib/inference";
  import {
    getLocalmailConnectionStatus,
    localmailConnectionTestMessage,
    probeLocalmailConnection,
    testLocalmailConnection,
    updateLocalmailConnection,
    type LocalmailConnectionStatus,
    type LocalmailProbeResult,
  } from "$lib/localmail";

  type Props = { disabled: boolean };

  let { disabled }: Props = $props();
  let initialized = false;
  let origin = $state("");
  let trustedOrigin = $state("");
  let trustedCertificate = $state("");
  let inspected = $state<LocalmailProbeResult | null>(null);
  let apiKey = $state("");
  let removeApiKey = $state(false);
  let credentialConfigured = $state(false);
  let credentialUnlocked = $state(false);
  let biometricProtected = $state(false);
  let busy = $state<"loading" | "inspecting" | "saving" | "testing" | null>(null);
  let feedback = $state("");
  let failed = $state(false);

  $effect(() => {
    if (!initialized) {
      initialized = true;
      void loadStatus();
    }
  });

  /** Copies secret-free native status into the connector presentation state. */
  function applyStatus(status: LocalmailConnectionStatus): void {
    origin = status.origin ?? "";
    trustedOrigin = status.origin ?? "";
    trustedCertificate = status.certificateSha256 ?? "";
    credentialConfigured = status.credentialConfigured;
    credentialUnlocked = status.credentialUnlocked;
    biometricProtected = status.biometricProtected;
  }

  /** Loads only path-free connection and credential availability. */
  async function loadStatus(): Promise<void> {
    if (!isTauri()) return;
    busy = "loading";
    try {
      applyStatus(await getLocalmailConnectionStatus());
    } catch (error) {
      showError(error);
    } finally {
      busy = null;
    }
  }

  /** Invalidates an inspected draft as soon as its origin changes. */
  function changeOrigin(value: string): void {
    origin = value;
    inspected = null;
    feedback = "";
  }

  /** Formats the exact fingerprint into readable groups without changing its value. */
  function displayFingerprint(value: string): string {
    return value.match(/.{1,4}/g)?.join(" ") ?? value;
  }

  /** Returns the pin applicable to the current exact origin draft. */
  function activeCertificate(): string | null {
    if (inspected?.origin === origin) return inspected.certificateSha256;
    if (trustedOrigin === origin) return trustedCertificate || null;
    return null;
  }

  /** Inspects server identity and certificate without persisting either value. */
  async function inspectCertificate(): Promise<void> {
    if (!isTauri() || disabled || busy) return;
    busy = "inspecting";
    feedback = "";
    failed = false;
    try {
      inspected = await probeLocalmailConnection(origin);
      origin = inspected.origin;
      feedback = "Review the server identity and certificate fingerprint before confirming trust.";
    } catch (error) {
      showError(error);
    } finally {
      busy = null;
    }
  }

  /** Saves explicit certificate trust and one optional vault-key mutation. */
  async function saveConnection(): Promise<void> {
    const certificate = activeCertificate();
    if (!isTauri() || disabled || busy || !certificate) return;
    try {
      await persistConnection(certificate);
    } catch (error) {
      showError(error);
    }
  }

  /** Includes pending key edits in the dialog save without implicitly trusting a new certificate. */
  export async function savePendingChanges(): Promise<void> {
    if (!isTauri()) return;
    if (busy) throw new Error("Wait for the Localmail operation to finish before saving Settings.");
    if (!apiKey.trim() && !removeApiKey && origin === trustedOrigin && !inspected) return;
    if (origin !== trustedOrigin || inspected || !trustedCertificate) {
      throw new Error("Confirm the Localmail certificate trust before saving Settings.");
    }
    await persistConnection(trustedCertificate);
  }

  /** Clears replacement drafts only after the native vault acknowledges the write. */
  async function persistConnection(certificate: string): Promise<void> {
    busy = "saving";
    feedback = "";
    failed = false;
    try {
      const status = await updateLocalmailConnection(origin, certificate, apiKey.trim() || null, removeApiKey);
      applyStatus(status);
      inspected = null;
      apiKey = "";
      removeApiKey = false;
      feedback = status.credentialConfigured
        ? "Certificate trust and the vault-held API key are saved."
        : "Certificate trust is saved. Add an API key to verify authentication.";
    } finally {
      busy = null;
    }
  }

  /** Tests only pinned identity and optional authentication against fixed Localmail endpoints. */
  async function testConnection(): Promise<void> {
    const certificate = activeCertificate();
    if (!isTauri() || disabled || busy || !certificate) return;
    busy = "testing";
    feedback = "";
    failed = false;
    try {
      const testedDraftKey = apiKey.trim().length > 0;
      const result = await testLocalmailConnection(origin, certificate, apiKey.trim() || null);
      feedback = localmailConnectionTestMessage(result, testedDraftKey, credentialConfigured);
    } catch (error) {
      showError(error);
    } finally {
      busy = null;
    }
  }

  /** Converts one native failure into the existing bounded provider-error presentation. */
  function showError(error: unknown): void {
    feedback = providerErrorFromUnknown(error).message;
    failed = true;
  }
</script>

<section class="provider-setting localmail-setting" aria-labelledby="localmail-settings-title">
  <div class="provider-setting-heading">
    <span>
      <strong id="localmail-settings-title">Localmail archive</strong>
      <small>First-party read-only connector foundation</small>
    </span>
    <span class="local-badge cloud"><Icon name="shield" size={12} /> Explicit HTTPS</span>
  </div>

  <label for="localmail-origin">HTTPS origin</label>
  <div class="endpoint-row">
    <input
      id="localmail-origin"
      type="url"
      value={origin}
      placeholder="https://localmail.example:8443"
      oninput={(event) => changeOrigin(event.currentTarget.value)}
      disabled={!isTauri() || disabled || busy !== null}
      autocomplete="url"
      spellcheck="false"
    />
    <button
      type="button"
      disabled={!isTauri() || disabled || busy !== null || !origin.trim()}
      onclick={inspectCertificate}
    >
      {busy === "inspecting" ? "Inspecting…" : "Inspect certificate"}
    </button>
  </div>

  {#if inspected}
    <div class="localmail-certificate" aria-live="polite">
      <span>Localmail {inspected.serverVersion} · API {inspected.apiMajor}.{inspected.apiMinor}</span>
      <code>{displayFingerprint(inspected.certificateSha256)}</code>
      <p>Compare this SHA-256 fingerprint with the Localmail administrator through a separate trusted channel.</p>
    </div>
  {:else if trustedCertificate && trustedOrigin === origin}
    <div class="localmail-certificate saved">
      <span>Trusted certificate SHA-256</span>
      <code>{displayFingerprint(trustedCertificate)}</code>
    </div>
  {/if}

  <label for="localmail-api-key">API key</label>
  <div class="credential-row">
    <input
      id="localmail-api-key"
      type="password"
      value={apiKey}
      placeholder={credentialConfigured ? "Stored in OS credential vault" : "Paste a Localmail lmk_ API key"}
      oninput={(event) => {
        apiKey = event.currentTarget.value;
        removeApiKey = false;
      }}
      disabled={!isTauri() || disabled || busy !== null}
      autocomplete="new-password"
      spellcheck="false"
    />
    <button
      type="button"
      class:pending={removeApiKey}
      disabled={!credentialConfigured || disabled || busy !== null}
      onclick={() => {
        removeApiKey = !removeApiKey;
        if (removeApiKey) apiKey = "";
      }}>{removeApiKey ? "Keep" : "Remove"}</button
    >
  </div>
  <p class="credential-status">
    {removeApiKey
      ? "The saved API key will be removed when this connection is saved."
      : apiKey
        ? "Save this connection or Save and reconnect to store the replacement API key in the operating-system credential vault."
        : credentialConfigured && biometricProtected && credentialUnlocked
          ? "The vault API key is unlocked for this Bottie session."
          : credentialConfigured && biometricProtected
            ? "The saved API key unlocks with Touch ID or your login password."
            : credentialConfigured
              ? "An API key is configured in the operating-system credential vault."
              : "No API key is configured."}
  </p>

  <p class="localmail-boundary">
    Use an API key beginning with <code>lmk_</code> from your Localmail administrator. Login tokens expire.
  </p>

  <p class="localmail-boundary">
    No email is read during setup. The API key stays in the operating-system credential vault. Inspect certificate, then
    Confirm certificate trust; Test calls only <code>/v1/version</code> and, when an API key exists,
    <code>/v1/auth/whoami</code>.
  </p>

  <div class="localmail-actions">
    <button
      type="button"
      disabled={!isTauri() || disabled || busy !== null || !activeCertificate()}
      onclick={testConnection}>{busy === "testing" ? "Testing…" : "Test connection"}</button
    >
    <button
      type="button"
      class="primary"
      disabled={!isTauri() || disabled || busy !== null || !activeCertificate()}
      onclick={saveConnection}>{busy === "saving" ? "Saving…" : "Confirm certificate trust and save"}</button
    >
  </div>

  {#if feedback}
    <p class:error={failed} class:success={!failed} class="test-result" role={failed ? "alert" : "status"}>
      {feedback}
    </p>
  {/if}
</section>
