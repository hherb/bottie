<script lang="ts">
  import { onMount } from "svelte";
  import { isTauri } from "@tauri-apps/api/core";

  import Icon from "$lib/Icon.svelte";
  import {
    getSemanticIndexProgress,
    memoryIndexCopy,
    memoryIndexIsActive,
    memoryIndexPercent,
    reindexSemanticMemory,
    type SemanticIndexProgress,
  } from "$lib/memory-index";

  type Props = {
    disabled: boolean;
  };

  const ACTIVE_REFRESH_INTERVAL_MS = 750;

  let { disabled }: Props = $props();
  let progress = $state<SemanticIndexProgress | null>(null);
  let error = $state("");
  let isSubmitting = $state(false);
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;
  const native = isTauri();

  onMount(() => {
    if (native) void refresh();
    return () => clearRefreshTimer();
  });

  /** Cancels the one pending native progress refresh. */
  function clearRefreshTimer(): void {
    if (refreshTimer !== undefined) clearTimeout(refreshTimer);
    refreshTimer = undefined;
  }

  /** Continues polling only while durable native work is active. */
  function scheduleRefresh(): void {
    clearRefreshTimer();
    if (progress && memoryIndexIsActive(progress)) {
      refreshTimer = setTimeout(() => void refresh(), ACTIVE_REFRESH_INTERVAL_MS);
    }
  }

  /** Loads path-free durable progress from the native store. */
  async function refresh(): Promise<void> {
    try {
      progress = await getSemanticIndexProgress();
      error = "";
    } catch {
      error = "Bottie could not read semantic index progress.";
    }
    scheduleRefresh();
  }

  /** Resets derived vectors through the restore-safe native command. */
  async function reindex(): Promise<void> {
    if (!native || disabled || isSubmitting || (progress && memoryIndexIsActive(progress))) return;
    isSubmitting = true;
    error = "";
    try {
      progress = await reindexSemanticMemory();
    } catch {
      error = "Bottie could not start semantic reindexing.";
    } finally {
      isSubmitting = false;
      scheduleRefresh();
    }
  }
</script>

<section class="memory-index" aria-labelledby="memory-index-title">
  <div class="memory-index-heading">
    <span class="memory-index-icon"><Icon name="brain" size={15} /></span>
    <span>
      <strong id="memory-index-title">Semantic memory index</strong>
      <small>Built locally from eligible messages and extracted documents</small>
    </span>
  </div>

  {#if progress}
    <div
      class="memory-progress"
      role="progressbar"
      aria-label="Semantic memory indexing progress"
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={memoryIndexPercent(progress)}
    >
      <span style={`width: ${memoryIndexPercent(progress)}%`}></span>
    </div>
    <p class:failed={progress.state === "failed"} class="memory-status" aria-live="polite">
      {memoryIndexCopy(progress)}
    </p>
  {:else if native && !error}
    <p class="memory-status" aria-live="polite">Reading durable index progress…</p>
  {/if}

  <div class="memory-index-actions">
    <p>Reindexing replaces derived vectors only. Source content and the app-owned model cache are retained.</p>
    <button
      type="button"
      disabled={!native || disabled || isSubmitting || Boolean(progress && memoryIndexIsActive(progress))}
      onclick={() => void reindex()}
    >
      {isSubmitting ? "Starting…" : "Reindex memory"}
    </button>
  </div>

  {#if !native}
    <p class="memory-error">Semantic indexing is available only in the native app.</p>
  {:else if error}
    <p class="memory-error" role="alert">{error}</p>
  {/if}
</section>

<style>
  .memory-index {
    padding: 14px;
    margin: 13px 0;
    border: 1px solid rgba(143, 125, 247, 0.16);
    border-radius: 12px;
    background: rgba(143, 125, 247, 0.035);
  }
  .memory-index-heading,
  .memory-index-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .memory-index-heading > span:last-child {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
  }
  .memory-index-icon {
    display: grid;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 8px;
    background: rgba(143, 125, 247, 0.12);
    color: #a99cf1;
  }
  strong {
    color: #dbd8e0;
    font-size: 11px;
    font-weight: 620;
  }
  small {
    margin-top: 3px;
    color: #706d7b;
    font-size: 8px;
  }
  .memory-progress {
    height: 4px;
    margin-top: 13px;
    overflow: hidden;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.06);
  }
  .memory-progress span {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, #796bd4, #5bd8c8);
    transition: width 180ms ease;
  }
  .memory-status,
  .memory-error {
    margin: 7px 0 0;
    color: #898692;
    font-size: 8.5px;
  }
  .memory-status.failed,
  .memory-error {
    color: #d89a91;
  }
  .memory-index-actions {
    align-items: flex-end;
    margin-top: 12px;
  }
  .memory-index-actions p {
    max-width: 365px;
    margin: 0;
    color: #6f6c79;
    font-size: 8px;
    line-height: 1.45;
  }
  button {
    min-width: 106px;
    height: 30px;
    padding: 0 10px;
    border: 1px solid rgba(143, 125, 247, 0.28);
    border-radius: 8px;
    background: rgba(143, 125, 247, 0.08);
    color: #b8afea;
    cursor: pointer;
    font-size: 8.5px;
    font-weight: 610;
  }
  button:hover {
    background: rgba(143, 125, 247, 0.14);
    color: #d2ccf3;
  }
  button:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }
  @media (max-width: 600px) {
    .memory-index-actions {
      align-items: stretch;
      flex-direction: column;
    }
    button {
      align-self: flex-end;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .memory-progress span {
      transition: none;
    }
  }
</style>
