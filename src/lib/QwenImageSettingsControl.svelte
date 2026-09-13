<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";

  import Icon from "$lib/Icon.svelte";
  import {
    providerErrorFromUnknown,
    validateQwenImageConfiguration,
    type ImageGenerationSetupStatus,
    type ProviderCredentialStatus,
  } from "$lib/inference";

  type ValidationState = {
    status: "idle" | "testing" | "success" | "error";
    message: string;
  };

  type Props = {
    baseUrl: string;
    credential: ProviderCredentialStatus;
    credentialDraft: string;
    removeCredential: boolean;
    disabled: boolean;
    onbaseurlchange: (baseUrl: string) => void;
    oncredentialdraftchange: (apiKey: string) => void;
    onremovecredentialchange: (remove: boolean) => void;
    ondiagnosticschange: () => Promise<void>;
  };

  let {
    baseUrl,
    credential,
    credentialDraft,
    removeCredential,
    disabled,
    onbaseurlchange,
    oncredentialdraftchange,
    onremovecredentialchange,
    ondiagnosticschange,
  }: Props = $props();
  let validation = $state<ValidationState>({ status: "idle", message: "" });
  let capabilities = $state<ImageGenerationSetupStatus | null>(null);

  /** Clears stale capability evidence after the user changes setup. */
  function clearValidation(): void {
    validation = { status: "idle", message: "" };
    capabilities = null;
  }

  /** Validates the fixed model route without issuing a billable provider request. */
  async function validateSetup(): Promise<void> {
    validation = { status: "testing", message: "Validating setup…" };
    capabilities = null;
    try {
      const result = await validateQwenImageConfiguration(baseUrl, credentialDraft);
      onbaseurlchange(result.baseUrl);
      capabilities = result;
      validation = { status: "success", message: result.message };
    } catch (error) {
      validation = { status: "error", message: providerErrorFromUnknown(error).message };
    }
    await ondiagnosticschange();
  }
</script>

<div class="provider-setting">
  <div class="provider-setting-heading">
    <span>
      <strong>Qwen-Image-2.0</strong>
      <small>Exact unified generation and editing model</small>
    </span>
    <span class="local-badge cloud">
      <Icon name="shield" size={12} />
      Cloud
    </span>
  </div>
  <p class="credential-status"><code>qwen-image-2.0-2026-03-03</code></p>
  <label for="qwen-image-api-key">Model Studio API key</label>
  <div class="credential-row">
    <input
      id="qwen-image-api-key"
      type="password"
      value={credentialDraft}
      placeholder={credential.configured ? "Stored in OS credential vault" : "Enter API key"}
      oninput={(event) => {
        oncredentialdraftchange(event.currentTarget.value);
        clearValidation();
      }}
      disabled={!isTauri() || disabled}
      autocomplete="new-password"
      spellcheck="false"
    />
    <button
      type="button"
      class:pending={removeCredential}
      disabled={!isTauri() || !credential.configured || disabled}
      onclick={() => {
        onremovecredentialchange(!removeCredential);
        clearValidation();
      }}>{removeCredential ? "Keep" : "Remove"}</button
    >
  </div>
  <p class="credential-status">
    {removeCredential
      ? "Credential will be removed when saved."
      : credentialDraft
        ? "Replacement key will be stored securely when saved."
        : credential.configured && credential.biometricProtected && credential.unlocked
          ? "Touch ID verified; credential unlocked for this Bottie session."
          : credential.configured && credential.biometricProtected
            ? "Protected by Touch ID; Bottie unlocks saved credentials together at app start."
            : credential.configured
              ? "Credential configured in the OS vault."
              : "No credential configured."}
  </p>
  <label for="qwen-image-endpoint">Endpoint</label>
  <div class="endpoint-row">
    <input
      id="qwen-image-endpoint"
      value={baseUrl}
      oninput={(event) => {
        onbaseurlchange(event.currentTarget.value);
        clearValidation();
      }}
      disabled={!isTauri() || disabled}
      spellcheck="false"
      autocomplete="off"
    />
    <button
      type="button"
      disabled={!isTauri() || disabled || validation.status === "testing" || (removeCredential && !credentialDraft)}
      onclick={validateSetup}>Validate setup</button
    >
  </div>
  <p class="credential-status">
    Singapore is the default region; documented Singapore and Beijing API roots are accepted. Validation checks the
    endpoint, fixed model identity, and credential shape; no image is generated or billed.
  </p>
  {#if capabilities}
    <p class="credential-status">
      Generation and editing · up to {capabilities.maxOutputs} outputs · up to
      {capabilities.maxPixels.toLocaleString()} pixels
    </p>
  {/if}
  {#if validation.message}
    <p
      class:error={validation.status === "error"}
      class:success={validation.status === "success"}
      class="test-result"
      role={validation.status === "error" ? "alert" : "status"}
    >
      {validation.message}
    </p>
  {/if}
</div>
