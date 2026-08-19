<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";

  import Icon from "$lib/Icon.svelte";
  import { DEFAULT_LOCAL_PROVIDER_SETTINGS } from "$lib/presentation";
  import {
    getDiagnostics,
    providerErrorFromUnknown,
    testProviderConnection,
    updateProviderSettings,
    type DiagnosticEntry,
    type LocalProviderId,
    type ProviderSettings,
  } from "$lib/inference";

  type ConnectionTestState = {
    status: "idle" | "testing" | "success" | "error";
    message: string;
  };

  type Props = {
    settings: ProviderSettings;
    isGenerating: boolean;
    onclose: () => void;
    onsaved: (settings: ProviderSettings) => Promise<void>;
  };

  /** Provider metadata rendered by the local-settings form. */
  const PROVIDER_SETTINGS: Array<{
    id: LocalProviderId;
    name: string;
    description: string;
  }> = [
    { id: "omlx", name: "oMLX", description: "OpenAI-compatible local runtime" },
    { id: "ollama", name: "Ollama", description: "Native local API" },
  ];

  let { settings, isGenerating, onclose, onsaved }: Props = $props();
  let settingsDraft = $state<ProviderSettings>({ ...DEFAULT_LOCAL_PROVIDER_SETTINGS });
  let draftInitialized = false;
  let settingsError = $state("");
  let settingsSaving = $state(false);
  let diagnostics = $state<DiagnosticEntry[]>([]);
  let connectionTests = $state<Record<LocalProviderId, ConnectionTestState>>({
    omlx: { status: "idle", message: "" },
    ollama: { status: "idle", message: "" },
  });

  $effect(() => {
    if (!draftInitialized) {
      settingsDraft = { ...settings };
      draftInitialized = true;
    }
    void refreshDiagnostics();
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

  /** Returns the draft endpoint for one local provider. */
  function endpoint(providerId: LocalProviderId): string {
    return providerId === "omlx" ? settingsDraft.omlxBaseUrl : settingsDraft.ollamaBaseUrl;
  }

  /** Updates the draft endpoint for one local provider. */
  function updateEndpoint(providerId: LocalProviderId, baseUrl: string): void {
    if (providerId === "omlx") settingsDraft.omlxBaseUrl = baseUrl;
    else settingsDraft.ollamaBaseUrl = baseUrl;
  }

  /** Closes the dialog unless a settings write is active. */
  function close(): void {
    if (!settingsSaving) onclose();
  }

  /** Tests one draft endpoint without changing the active provider configuration. */
  async function testConnection(providerId: LocalProviderId): Promise<void> {
    const baseUrl = providerId === "omlx" ? settingsDraft.omlxBaseUrl : settingsDraft.ollamaBaseUrl;
    connectionTests[providerId] = { status: "testing", message: "Testing connection…" };
    settingsError = "";
    try {
      const result = await testProviderConnection(providerId, baseUrl);
      connectionTests[providerId] = {
        status: "success",
        message: `${result.message} ${result.elapsedMs} ms.`,
      };
      if (providerId === "omlx") settingsDraft.omlxBaseUrl = result.baseUrl;
      else settingsDraft.ollamaBaseUrl = result.baseUrl;
    } catch (error) {
      const normalized = providerErrorFromUnknown(error);
      connectionTests[providerId] = { status: "error", message: normalized.message };
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
        <span class="eyebrow">Rust-owned configuration</span>
        <h2 id="provider-settings-title">Local providers</h2>
      </div>
      <button class="icon-button" aria-label="Close provider settings" onclick={close}>
        <Icon name="x" size={18} />
      </button>
    </header>

    <form class="settings-content" onsubmit={save}>
      <p class="settings-intro">
        Bottie accepts loopback endpoints only. Provider traffic and configuration stay behind the native boundary.
      </p>

      {#each PROVIDER_SETTINGS as provider}
        {@const test = connectionTests[provider.id]}
        <div class="provider-setting">
          <div class="provider-setting-heading">
            <span><strong>{provider.name}</strong><small>{provider.description}</small></span>
            <span class="local-badge"><Icon name="shield" size={12} /> Local</span>
          </div>
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

      <div class="settings-policy">
        <Icon name="shield" size={15} />
        <span
          ><strong>Connection policy</strong><small>3 s connect · 5 s discovery · 120 s stream idle timeout</small
          ></span
        >
      </div>

      <section class="diagnostics" aria-label="Recent provider diagnostics">
        <div class="diagnostics-heading">
          <span><strong>Recent diagnostics</strong><small>Structured and secret-redacted</small></span>
          <button type="button" onclick={refreshDiagnostics}>Refresh</button>
        </div>
        {#if diagnostics.length === 0}
          <p class="diagnostics-empty">No provider activity has been recorded this session.</p>
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
