<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";

  import AppearancePreferences from "$lib/AppearancePreferences.svelte";
  import Icon from "$lib/Icon.svelte";
  import MemoryIndexControl from "$lib/MemoryIndexControl.svelte";
  import LocalmailSettingsControl from "$lib/LocalmailSettingsControl.svelte";
  import ConversationRetentionControl from "$lib/ConversationRetentionControl.svelte";
  import WebNetworkPolicyControl from "$lib/WebNetworkPolicyControl.svelte";
  import { DEFAULT_PROVIDER_SETTINGS } from "$lib/presentation";
  import {
    exportDiagnostics,
    getDiagnostics,
    getProviderCredentialStatus,
    providerErrorFromUnknown,
    testProviderConnection,
    testWebSearchConnection,
    updateProviderCredential,
    updateProviderSettings,
    type CredentialProviderId,
    type DiagnosticEntry,
    type ProviderCredentialStatus,
    type ProviderId,
    type ProviderSettings,
    type WebSearchProviderId,
  } from "$lib/inference";
  import { cloneWebNetworkPolicy } from "$lib/web-policy";
  import type { AppearancePreferences as AppearancePreferenceValues } from "$lib/appearance";
  import { CONNECTION_POLICY, PROVIDER_SETTINGS, SEARCH_PROVIDER_SETTINGS } from "$lib/provider-settings-options";

  type ConnectionTestState = {
    status: "idle" | "testing" | "success" | "error";
    message: string;
  };

  type Props = {
    settings: ProviderSettings;
    appearance: AppearancePreferenceValues;
    isGenerating: boolean;
    onclose: () => void;
    onappearancechange: (appearance: AppearancePreferenceValues) => void;
    onsaved: (settings: ProviderSettings) => Promise<void>;
  };

  let { settings, appearance, isGenerating, onclose, onappearancechange, onsaved }: Props = $props();
  let settingsDraft = $state<ProviderSettings>({
    ...DEFAULT_PROVIDER_SETTINGS,
    webNetworkPolicy: cloneWebNetworkPolicy(DEFAULT_PROVIDER_SETTINGS.webNetworkPolicy),
  });
  let draftInitialized = false;
  let settingsError = $state("");
  let settingsSaving = $state(false);
  let diagnostics = $state<DiagnosticEntry[]>([]);
  let diagnosticsExporting = $state(false);
  let diagnosticsExportFeedback = $state("");
  let diagnosticsExportFailed = $state(false);
  let connectionTests = $state<Record<ProviderId, ConnectionTestState>>({
    omlx: { status: "idle", message: "" },
    ollama: { status: "idle", message: "" },
    openai: { status: "idle", message: "" },
    anthropic: { status: "idle", message: "" },
  });
  let webSearchTests = $state<Record<WebSearchProviderId, ConnectionTestState>>({
    brave: { status: "idle", message: "" },
    exa: { status: "idle", message: "" },
  });
  let credentialStatus = $state<Record<CredentialProviderId, ProviderCredentialStatus>>({
    openai: { providerId: "openai", configured: false, unlocked: false, biometricProtected: false },
    anthropic: { providerId: "anthropic", configured: false, unlocked: false, biometricProtected: false },
    brave: { providerId: "brave", configured: false, unlocked: false, biometricProtected: false },
    exa: { providerId: "exa", configured: false, unlocked: false, biometricProtected: false },
  });
  let credentialDrafts = $state<Record<CredentialProviderId, string>>({
    openai: "",
    anthropic: "",
    brave: "",
    exa: "",
  });
  let removeCredentials = $state<Record<CredentialProviderId, boolean>>({
    openai: false,
    anthropic: false,
    brave: false,
    exa: false,
  });

  $effect(() => {
    if (!draftInitialized) {
      settingsDraft = {
        ...settings,
        webNetworkPolicy: cloneWebNetworkPolicy(settings.webNetworkPolicy),
      };
      draftInitialized = true;
    }
    void refreshDiagnostics();
    void refreshCredentialStatus();
  });

  /** Returns a compact local time for one diagnostic timestamp. */
  function diagnosticTime(timestampMs: number): string {
    return new Date(timestampMs).toLocaleTimeString([], {
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  /** Refreshes the secret-redacted session diagnostic list. */
  async function refreshDiagnostics(): Promise<void> {
    diagnostics = await getDiagnostics().catch(() => diagnostics);
  }

  /** Opens the native diagnostic Save flow and reports only its path-free outcome. */
  async function saveDiagnostics(): Promise<void> {
    if (!isTauri() || diagnostics.length === 0 || diagnosticsExporting) return;
    diagnosticsExporting = true;
    diagnosticsExportFeedback = "";
    diagnosticsExportFailed = false;
    try {
      const outcome = await exportDiagnostics();
      if (outcome.status === "saved") {
        diagnosticsExportFeedback = `Saved redacted diagnostics as ${outcome.fileName ?? "JSON"}.`;
      } else {
        diagnosticsExportFeedback = "Diagnostics export cancelled.";
      }
    } catch (error) {
      diagnosticsExportFeedback = providerErrorFromUnknown(error).message;
      diagnosticsExportFailed = true;
    } finally {
      diagnosticsExporting = false;
    }
  }

  /** Refreshes remote credential availability without reading secret values. */
  async function refreshCredentialStatus(): Promise<void> {
    const statuses = await getProviderCredentialStatus().catch(() => [] as ProviderCredentialStatus[]);
    for (const status of statuses) credentialStatus[status.providerId] = status;
  }

  /** Returns the draft endpoint for one provider. */
  function endpoint(providerId: ProviderId): string {
    if (providerId === "omlx") return settingsDraft.omlxBaseUrl;
    if (providerId === "ollama") return settingsDraft.ollamaBaseUrl;
    if (providerId === "openai") return settingsDraft.openaiBaseUrl;
    return settingsDraft.anthropicBaseUrl;
  }

  /** Updates the draft endpoint for one provider. */
  function updateEndpoint(providerId: ProviderId, baseUrl: string): void {
    if (providerId === "omlx") settingsDraft.omlxBaseUrl = baseUrl;
    else if (providerId === "ollama") settingsDraft.ollamaBaseUrl = baseUrl;
    else if (providerId === "openai") settingsDraft.openaiBaseUrl = baseUrl;
    else settingsDraft.anthropicBaseUrl = baseUrl;
  }

  /** Closes the dialog unless a settings write is active. */
  function close(): void {
    if (!settingsSaving) onclose();
  }

  /** Tests one draft endpoint without changing the active provider configuration. */
  async function testConnection(providerId: ProviderId): Promise<void> {
    const baseUrl = endpoint(providerId);
    connectionTests[providerId] = { status: "testing", message: "Testing connection…" };
    settingsError = "";
    try {
      const apiKey = providerId === "openai" || providerId === "anthropic" ? credentialDrafts[providerId] : undefined;
      const result = await testProviderConnection(providerId, baseUrl, apiKey);
      connectionTests[providerId] = {
        status: "success",
        message: `${result.message} ${result.elapsedMs} ms.`,
      };
      updateEndpoint(providerId, result.baseUrl);
    } catch (error) {
      const normalized = providerErrorFromUnknown(error);
      connectionTests[providerId] = { status: "error", message: normalized.message };
    }
    await refreshDiagnostics();
  }

  /** Tests one fixed search route without saving or returning provider results. */
  async function testSearchConnection(providerId: WebSearchProviderId): Promise<void> {
    webSearchTests[providerId] = { status: "testing", message: "Testing connection…" };
    settingsError = "";
    try {
      const result = await testWebSearchConnection(providerId, credentialDrafts[providerId]);
      webSearchTests[providerId] = {
        status: "success",
        message: `${result.message} ${result.elapsedMs} ms.`,
      };
    } catch (error) {
      const normalized = providerErrorFromUnknown(error);
      webSearchTests[providerId] = { status: "error", message: normalized.message };
    }
    await refreshDiagnostics();
  }

  /** Persists validated provider settings and asks the parent shell to rediscover models. */
  async function save(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (!isTauri() || isGenerating || settingsSaving) return;
    settingsSaving = true;
    settingsError = "";
    try {
      const saved = await updateProviderSettings({ ...settingsDraft });
      for (const providerId of ["openai", "anthropic", "brave", "exa"] as const) {
        const apiKey = credentialDrafts[providerId].trim();
        if (apiKey || removeCredentials[providerId]) {
          const status = await updateProviderCredential(providerId, apiKey || null, removeCredentials[providerId]);
          credentialStatus[providerId] = status;
        }
      }
      await onsaved(saved);
      onclose();
    } catch (error) {
      settingsError = providerErrorFromUnknown(error).message;
    } finally {
      settingsSaving = false;
    }
  }
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && close()} />

<div class="settings-layer">
  <button class="settings-scrim" aria-label="Close provider settings" onclick={close}></button>
  <div class="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="provider-settings-title">
    <header class="settings-header">
      <div>
        <span class="eyebrow">Local and native configuration</span>
        <h2 id="provider-settings-title">Settings</h2>
      </div>
      <button class="icon-button" aria-label="Close provider settings" onclick={close}>
        <Icon name="x" size={18} />
      </button>
    </header>

    <form class="settings-content" onsubmit={save}>
      <AppearancePreferences {appearance} onchange={onappearancechange} />

      <p class="settings-intro">
        Local routes require loopback endpoints. Cloud routes require HTTPS and keep API keys in the operating-system
        credential vault; keys are never returned to the interface.
      </p>

      {#each PROVIDER_SETTINGS as provider}
        {@const test = connectionTests[provider.id]}
        {@const remoteProviderId = provider.id === "openai" || provider.id === "anthropic" ? provider.id : null}
        {@const credential = remoteProviderId ? credentialStatus[remoteProviderId] : null}
        <div class="provider-setting">
          <div class="provider-setting-heading">
            <span><strong>{provider.name}</strong><small>{provider.description}</small></span>
            <span class:cloud={provider.route === "cloud"} class="local-badge">
              <Icon name="shield" size={12} />
              {provider.route === "local" ? "Local" : "Cloud"}
            </span>
          </div>
          {#if remoteProviderId}
            <label for={`${provider.id}-api-key`}>API key</label>
            <div class="credential-row">
              <input
                id={`${provider.id}-api-key`}
                type="password"
                value={credentialDrafts[remoteProviderId]}
                placeholder={credential?.configured ? "Stored in OS credential vault" : "Enter API key"}
                oninput={(event) => {
                  credentialDrafts[remoteProviderId] = event.currentTarget.value;
                  removeCredentials[remoteProviderId] = false;
                }}
                disabled={!isTauri() || settingsSaving}
                autocomplete="new-password"
                spellcheck="false"
              />
              <button
                type="button"
                class:pending={removeCredentials[remoteProviderId]}
                disabled={!credential?.configured || settingsSaving}
                onclick={() => (removeCredentials[remoteProviderId] = !removeCredentials[remoteProviderId])}
                >{removeCredentials[remoteProviderId] ? "Keep" : "Remove"}</button
              >
            </div>
            <p class="credential-status">
              {removeCredentials[remoteProviderId]
                ? "Credential will be removed when saved."
                : credentialDrafts[remoteProviderId]
                  ? "Replacement key will be stored securely when saved."
                  : credential?.configured && credential.biometricProtected && credential.unlocked
                    ? "Touch ID verified; credential unlocked for this Bottie session."
                    : credential?.configured && credential.biometricProtected
                      ? "Protected by Touch ID; unlocks on first use this session."
                      : credential?.configured
                        ? "Credential configured in the OS vault."
                        : "No credential configured."}
            </p>
          {/if}
          <label for={`${provider.id}-endpoint`}>Endpoint</label>
          <div class="endpoint-row">
            <input
              id={`${provider.id}-endpoint`}
              value={endpoint(provider.id)}
              oninput={(event) => updateEndpoint(provider.id, event.currentTarget.value)}
              disabled={!isTauri() || settingsSaving}
              spellcheck="false"
              autocomplete="off"
            />
            <button
              type="button"
              disabled={!isTauri() || settingsSaving || connectionTests[provider.id].status === "testing"}
              onclick={() => testConnection(provider.id)}>Test</button
            >
          </div>
          {#if test.message}
            <p class:error={test.status === "error"} class:success={test.status === "success"} class="test-result">
              {test.message}
            </p>
          {/if}
        </div>
      {/each}

      <div class="provider-setting">
        <div class="provider-setting-heading">
          <span><strong>Web search engine</strong><small>Used only when Web is explicitly enabled</small></span>
          <span class="local-badge cloud">
            <Icon name="shield" size={12} />
            Cloud
          </span>
        </div>
        <label for="web-search-provider">Search engine</label>
        <select
          id="web-search-provider"
          aria-label="Choose web search engine"
          value={settingsDraft.webSearchProviderId}
          disabled={!isTauri() || settingsSaving}
          onchange={(event) => (settingsDraft.webSearchProviderId = event.currentTarget.value as WebSearchProviderId)}
        >
          {#each SEARCH_PROVIDER_SETTINGS as provider}
            <option value={provider.id}>{provider.name}</option>
          {/each}
        </select>
        <p class="credential-status">
          Keys are stored independently, so changing engines does not remove either credential.
        </p>
      </div>

      {#each SEARCH_PROVIDER_SETTINGS as provider}
        {@const credential = credentialStatus[provider.id]}
        {@const test = webSearchTests[provider.id]}
        <div class="provider-setting">
          <div class="provider-setting-heading">
            <span><strong>{provider.name}</strong><small>Fixed native HTTPS search route</small></span>
            <span class="local-badge cloud">
              <Icon name="shield" size={12} />
              Cloud
            </span>
          </div>
          <label for={`${provider.id}-api-key`}>API key</label>
          <div class="credential-row">
            <input
              id={`${provider.id}-api-key`}
              type="password"
              value={credentialDrafts[provider.id]}
              placeholder={credential.configured ? "Stored in OS credential vault" : "Enter API key"}
              oninput={(event) => {
                credentialDrafts[provider.id] = event.currentTarget.value;
                removeCredentials[provider.id] = false;
              }}
              disabled={!isTauri() || settingsSaving}
              autocomplete="new-password"
              spellcheck="false"
            />
            <button
              type="button"
              class:pending={removeCredentials[provider.id]}
              disabled={!credential.configured || settingsSaving}
              onclick={() => (removeCredentials[provider.id] = !removeCredentials[provider.id])}
              >{removeCredentials[provider.id] ? "Keep" : "Remove"}</button
            >
          </div>
          <p class="credential-status">
            {removeCredentials[provider.id]
              ? "Credential will be removed when saved."
              : credentialDrafts[provider.id]
                ? "Replacement key will be stored securely when saved."
                : credential.configured && credential.biometricProtected && credential.unlocked
                  ? "Touch ID verified; credential unlocked for this Bottie session."
                  : credential.configured && credential.biometricProtected
                    ? "Protected by Touch ID; unlocks on first use this session."
                    : credential.configured
                      ? "Credential configured in the OS vault."
                      : "No credential configured."}
          </p>
          <div class="fixed-route-row">
            <span>{provider.hostname}</span>
            <button
              type="button"
              disabled={!isTauri() || settingsSaving || test.status === "testing"}
              onclick={() => testSearchConnection(provider.id)}>Test</button
            >
          </div>
          <p class="credential-status">
            Connection test sends one fixed bounded probe and does not expose search results.
          </p>
          {#if test.message}
            <p class:error={test.status === "error"} class:success={test.status === "success"} class="test-result">
              {test.message}
            </p>
          {/if}
        </div>
      {/each}

      <WebNetworkPolicyControl
        policy={settingsDraft.webNetworkPolicy}
        disabled={!isTauri() || settingsSaving}
        onchange={(policy) => (settingsDraft.webNetworkPolicy = policy)}
      />

      <LocalmailSettingsControl disabled={isGenerating || settingsSaving} />

      <div class="settings-policy">
        <Icon name="shield" size={15} />
        <span><strong>Connection policy</strong><small>{CONNECTION_POLICY}</small></span>
      </div>

      <MemoryIndexControl disabled={isGenerating || settingsSaving} />

      <ConversationRetentionControl disabled={isGenerating || settingsSaving} />

      <section class="diagnostics" aria-label="Recent native diagnostics">
        <div class="diagnostics-heading">
          <span><strong>Recent diagnostics</strong><small>Structured with secrets and paths redacted</small></span>
          <div class="diagnostics-actions">
            <button
              type="button"
              disabled={!isTauri() || diagnostics.length === 0 || diagnosticsExporting}
              onclick={saveDiagnostics}>{diagnosticsExporting ? "Exporting…" : "Export JSON"}</button
            >
            <button type="button" onclick={refreshDiagnostics}>Refresh</button>
          </div>
        </div>
        {#if diagnostics.length === 0}
          <p class="diagnostics-empty">No native activity has been recorded this session.</p>
        {:else}
          <div class="diagnostic-list">
            {#each diagnostics.slice(-6).reverse() as entry}
              <div class:error={entry.level === "error"} class="diagnostic-row">
                <span>{diagnosticTime(entry.timestampMs)}</span>
                <strong>{entry.event}</strong>
                <small>{entry.providerId ?? "native"}{entry.detail ? ` · ${entry.detail}` : ""}</small>
              </div>
            {/each}
          </div>
        {/if}
        {#if diagnosticsExportFeedback}
          <p
            class:error={diagnosticsExportFailed}
            class="diagnostics-export-feedback"
            role={diagnosticsExportFailed ? "alert" : "status"}
          >
            {diagnosticsExportFeedback}
          </p>
        {/if}
      </section>

      {#if !isTauri()}
        <p class="settings-error">Provider settings are read-only in the browser preview.</p>
      {:else if isGenerating}
        <p class="settings-error">Stop the active generation before changing provider settings.</p>
      {:else if settingsError}
        <p class="settings-error" role="alert">{settingsError}</p>
      {/if}

      <footer class="settings-actions">
        <button type="button" class="secondary" onclick={close}>Cancel</button>
        <button type="submit" class="primary" disabled={!isTauri() || isGenerating || settingsSaving}>
          {settingsSaving ? "Saving…" : "Save and reconnect"}
        </button>
      </footer>
    </form>
  </div>
</div>
