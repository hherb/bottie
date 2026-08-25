<script lang="ts">
  import { onMount } from "svelte";

  import Icon from "$lib/Icon.svelte";

  type Props = {
    providerName: string | null;
    modelName: string | null;
    providerEndpoint?: string | null;
    isLocalRoute: boolean;
    canComplete: boolean;
    isSaving: boolean;
    error: string;
    onopensettings: () => void;
    oncomplete: () => void;
  };

  let {
    providerName,
    modelName,
    providerEndpoint = null,
    isLocalRoute,
    canComplete,
    isSaving,
    error,
    onopensettings,
    oncomplete,
  }: Props = $props();
  let dialog: HTMLDivElement;
  let settingsButton: HTMLButtonElement;
  let completeButton: HTMLButtonElement;

  onMount(() => settingsButton.focus());

  /** Keeps keyboard navigation inside the first-run actions until setup or Settings takes over. */
  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      return;
    }
    if (event.key !== "Tab") return;
    if (isSaving) {
      event.preventDefault();
      dialog.focus();
      return;
    }
    const lastButton = canComplete && !isSaving ? completeButton : settingsButton;
    const activeElement = document.activeElement;
    if (activeElement !== settingsButton && activeElement !== completeButton) {
      event.preventDefault();
      settingsButton.focus();
    } else if (event.shiftKey && activeElement === settingsButton) {
      event.preventDefault();
      lastButton.focus();
    } else if (!event.shiftKey && activeElement === lastButton) {
      event.preventDefault();
      settingsButton.focus();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="first-run-layer">
  <div
    bind:this={dialog}
    class="first-run-card"
    role="dialog"
    aria-modal="true"
    aria-labelledby="first-run-title"
    tabindex="-1"
  >
    <header class="first-run-header">
      <span class="first-run-mark"><Icon name="shield" size={20} /></span>
      <div>
        <span class="eyebrow">Local-first by design</span>
        <h1 id="first-run-title">Before your first conversation</h1>
      </div>
    </header>

    <p class="first-run-intro">
      Bottie keeps storage and tools behind its native boundary. Confirm where a prompt will travel before continuing.
    </p>

    {#if providerName && modelName}
      <div class:cloud={!isLocalRoute} class="first-run-route">
        <span class="first-run-route-icon"><Icon name={isLocalRoute ? "database" : "globe"} size={18} /></span>
        <span>
          <small>{isLocalRoute ? "Local route" : "Cloud route"}</small>
          <strong>{providerName} · {modelName}</strong>
          {#if providerEndpoint}<em>{providerEndpoint}</em>{/if}
        </span>
      </div>
    {:else}
      <div class="first-run-offline" role="status">
        <Icon name="refresh" size={17} />
        <span
          ><strong>No streaming model is ready yet</strong><small>Start a local runtime or configure a provider.</small
          ></span
        >
      </div>
    {/if}

    <div class="first-run-disclosures">
      <article>
        <Icon name="arrow-up" size={16} />
        <span
          ><strong>Provider delivery</strong><small
            >Prompts, delivered images, and explicitly enabled tool results go to this provider.</small
          ></span
        >
      </article>
      <article>
        <Icon name="database" size={16} />
        <span
          ><strong>Local application data</strong><small
            >Conversations, files, and derived memory stay in Bottie’s local storage.</small
          ></span
        >
      </article>
      <article>
        <Icon name="shield" size={16} />
        <span
          ><strong>Explicit context</strong><small
            >Memory and Web start disabled, then remember choices that remain available.</small
          ></span
        >
      </article>
    </div>

    <p class="first-run-note">
      Cloud API keys and Web search keys stay in the operating-system credential vault. Finishing setup stores only this
      acknowledgement, your selected provider/model pair, and later Memory, Web, or Email choices.
    </p>

    {#if error}<p class="first-run-error" role="alert">{error}</p>{/if}

    <footer class="first-run-actions">
      <button bind:this={settingsButton} type="button" class="secondary" disabled={isSaving} onclick={onopensettings}
        >Open provider settings</button
      >
      <button
        bind:this={completeButton}
        type="button"
        class="primary"
        disabled={!canComplete || isSaving}
        onclick={oncomplete}
      >
        {isSaving ? "Finishing…" : "Finish setup"}
      </button>
    </footer>
  </div>
</div>
