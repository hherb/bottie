<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { modelKey } from "$lib/chat";
  import { PROVIDER_OPTIONS, type ProviderStatus } from "$lib/presentation";
  import type { ModelInfo, ProviderId, ReasoningEffort } from "$lib/inference";

  type Props = {
    providerId: ProviderId | "";
    selectedModelKey: string;
    models: ModelInfo[];
    providerStatus: ProviderStatus;
    isGenerating: boolean;
    reasoningEffort: ReasoningEffort;
    showContext: boolean;
    isLocalRoute: boolean;
    canExport: boolean;
    isExporting: boolean;
    exportFeedback: string | null;
    exportFailed: boolean;
    onproviderchange: (providerId: ProviderId) => void;
    onmodelchange: (modelKey: string) => void;
    ontogglereasoning: () => void;
    onopensidebar: () => void;
    ontogglecontext: () => void;
    onexport: () => void;
  };

  let {
    providerId,
    selectedModelKey,
    models,
    providerStatus,
    isGenerating,
    reasoningEffort,
    showContext,
    isLocalRoute,
    canExport,
    isExporting,
    exportFeedback,
    exportFailed,
    onproviderchange,
    onmodelchange,
    ontogglereasoning,
    onopensidebar,
    ontogglecontext,
    onexport,
  }: Props = $props();
</script>

<header class="topbar">
  <button class="icon-button mobile-menu" aria-label="Open conversations" onclick={onopensidebar}>
    <Icon name="menu" size={19} />
  </button>

  <div class="provider-selectors">
    <span
      class:checking={providerStatus === "checking"}
      class:offline={providerStatus === "offline" || providerStatus === "browser"}
      class="provider-pip"
    ></span>
    <label class="provider-pulldown">
      <span>Provider</span>
      <select
        value={providerId}
        disabled={providerStatus === "browser" || isGenerating}
        aria-label="Choose inference provider"
        onchange={(event) => onproviderchange(event.currentTarget.value as ProviderId)}
      >
        <option value="" disabled>{providerStatus === "browser" ? "Native only" : "Choose provider"}</option>
        {#each PROVIDER_OPTIONS as provider}
          <option value={provider.id}>{provider.name}{provider.route === "cloud" ? " · cloud" : " · local"}</option>
        {/each}
      </select>
    </label>
    <label class="model-pulldown">
      <span>Model</span>
      <select
        value={selectedModelKey}
        disabled={providerStatus !== "available" || isGenerating}
        aria-label="Choose model"
        onchange={(event) => onmodelchange(event.currentTarget.value)}
      >
        {#if models.length === 0}
          <option value="">
            {providerStatus === "checking"
              ? "Refreshing…"
              : providerStatus === "browser"
                ? "Native unavailable"
                : "No models available"}
          </option>
        {/if}
        {#each models as model (modelKey(model))}
          <option value={modelKey(model)}>
            {model.displayName}{model.loadState === "loaded"
              ? " · loaded"
              : model.loadState === "unloaded"
                ? " · on demand"
                : ""}
          </option>
        {/each}
      </select>
    </label>
    <button
      class:active={reasoningEffort === "low"}
      class="reasoning-toggle"
      type="button"
      role="switch"
      aria-checked={reasoningEffort === "low"}
      aria-label="Toggle thinking and reasoning"
      disabled={isGenerating}
      onclick={ontogglereasoning}
    >
      <Icon name="brain" size={13} />
      <span>Reasoning</span>
      <small>{reasoningEffort === "low" ? "Low" : "Off"}</small>
    </button>
  </div>

  <div class="topbar-actions">
    <div
      class:cloud={!isLocalRoute}
      class="privacy-pill"
      title={isLocalRoute ? "Messages stay on this device" : "Messages are sent to the selected cloud endpoint"}
    >
      <Icon name="shield" size={14} />
      <span>{isLocalRoute ? "Local only" : "Cloud route"}</span>
    </div>
    <button
      class:active={showContext}
      class="icon-button"
      aria-label="Toggle context panel"
      aria-pressed={showContext}
      onclick={ontogglecontext}
    >
      <Icon name="panel" size={18} />
    </button>
    {#if exportFeedback}
      <span class:error={exportFailed} class="export-feedback" role="status">{exportFeedback}</span>
    {/if}
    <button
      class="icon-button"
      aria-label={isExporting ? "Exporting conversation" : "Export conversation as Markdown"}
      title="Export conversation as Markdown"
      disabled={!canExport || isGenerating}
      onclick={onexport}
    >
      <Icon name="file" size={18} />
    </button>
  </div>
</header>
