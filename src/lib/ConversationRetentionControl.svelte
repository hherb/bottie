<script lang="ts">
  import { onMount } from "svelte";
  import { isTauri } from "@tauri-apps/api/core";

  import Icon from "$lib/Icon.svelte";
  import {
    CONVERSATION_RETENTION_OPTIONS,
    conversationRetentionDisclosure,
    getConversationRetentionPolicy,
    setConversationRetentionPeriod,
    type ConversationRetentionPeriod,
  } from "$lib/conversation-retention";

  type Props = {
    disabled: boolean;
  };

  let { disabled }: Props = $props();
  let savedPeriod = $state<ConversationRetentionPeriod>("forever");
  let draftPeriod = $state<ConversationRetentionPeriod>("forever");
  let loaded = $state(false);
  let saving = $state(false);
  let error = $state("");
  let feedback = $state("");
  const native = isTauri();

  onMount(() => {
    if (native) void load();
  });

  /** Loads the current path-free policy from native storage. */
  async function load(): Promise<void> {
    try {
      const policy = await getConversationRetentionPolicy();
      savedPeriod = policy.period;
      draftPeriod = policy.period;
      error = "";
    } catch {
      error = "Bottie could not read the Trash retention policy.";
    } finally {
      loaded = true;
    }
  }

  /** Persists one explicit period without applying deletion in the current process. */
  async function save(): Promise<void> {
    if (!native || disabled || saving || !loaded || draftPeriod === savedPeriod) return;
    saving = true;
    error = "";
    feedback = "";
    try {
      const policy = await setConversationRetentionPeriod(draftPeriod);
      savedPeriod = policy.period;
      draftPeriod = policy.period;
      feedback = "Trash retention saved. Expiry is checked on the next healthy app launch.";
    } catch {
      error = "Bottie could not save the Trash retention policy.";
    } finally {
      saving = false;
    }
  }
</script>

<section class="retention" aria-labelledby="retention-title">
  <div class="retention-heading">
    <span class="retention-icon"><Icon name="trash" size={15} /></span>
    <span>
      <strong id="retention-title">Trash retention</strong>
      <small>Opt-in permanent deletion based on time in Trash</small>
    </span>
  </div>

  <label for="conversation-retention-period">Retain conversations</label>
  <div class="retention-control">
    <select
      id="conversation-retention-period"
      bind:value={draftPeriod}
      disabled={!native || disabled || saving || !loaded}
      onchange={() => {
        feedback = "";
        error = "";
      }}
    >
      {#each CONVERSATION_RETENTION_OPTIONS as option}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
    <button
      type="button"
      disabled={!native || disabled || saving || !loaded || draftPeriod === savedPeriod}
      onclick={() => void save()}
    >
      {saving ? "Saving…" : "Save retention"}
    </button>
  </div>

  <p class="retention-disclosure">{conversationRetentionDisclosure(draftPeriod)}</p>
  <p class="retention-boundary">
    Automatic forget leaves existing exports and backups unchanged and retains the app-owned model cache.
  </p>
  {#if !native}
    <p class="retention-error">Trash retention is available only in the native app.</p>
  {:else if error}
    <p class="retention-error" role="alert">{error}</p>
  {:else if feedback}
    <p class="retention-feedback" role="status">{feedback}</p>
  {/if}
</section>

<style>
  .retention {
    padding: 14px;
    margin: 13px 0;
    border: 1px solid rgba(216, 154, 145, 0.18);
    border-radius: 12px;
    background: rgba(216, 154, 145, 0.035);
  }
  .retention-heading {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-bottom: 12px;
  }
  .retention-heading > span:last-child {
    display: flex;
    min-width: 0;
    flex: 1;
    flex-direction: column;
  }
  .retention-icon {
    display: grid;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 8px;
    background: rgba(216, 154, 145, 0.1);
    color: #d89a91;
  }
  strong,
  label {
    color: #dbd8e0;
    font-size: 11px;
    font-weight: 620;
  }
  small {
    margin-top: 3px;
    color: #706d7b;
    font-size: 8px;
  }
  label {
    display: block;
    margin-bottom: 6px;
    color: #8d8996;
    font-size: 8.5px;
  }
  .retention-control {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  select {
    height: 32px;
    min-width: 0;
    flex: 1;
    padding: 0 9px;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 8px;
    background: #18171d;
    color: #c8c5ce;
    font-size: 9px;
  }
  button {
    min-width: 106px;
    height: 32px;
    padding: 0 10px;
    border: 1px solid rgba(216, 154, 145, 0.28);
    border-radius: 8px;
    background: rgba(216, 154, 145, 0.08);
    color: #e0aaa2;
    cursor: pointer;
    font-size: 8.5px;
    font-weight: 610;
  }
  button:hover {
    background: rgba(216, 154, 145, 0.14);
    color: #efc2bc;
  }
  button:disabled,
  select:disabled {
    cursor: not-allowed;
    opacity: 0.48;
  }
  .retention-disclosure,
  .retention-boundary,
  .retention-error,
  .retention-feedback {
    margin: 9px 0 0;
    color: #777481;
    font-size: 8px;
    line-height: 1.5;
  }
  .retention-boundary {
    margin-top: 5px;
  }
  .retention-error {
    color: #d89a91;
  }
  .retention-feedback {
    color: #70c7b8;
  }
  @media (max-width: 600px) {
    .retention-control {
      align-items: stretch;
      flex-direction: column;
    }
    button {
      align-self: flex-end;
    }
  }
</style>
