<script lang="ts">
  import Icon from "$lib/Icon.svelte";

  type Props = {
    automaticBackupCount: number;
    latestAutomaticBackupAtMs: number | null;
    isRestoring: boolean;
    feedback: string | null;
    failed: boolean;
    onrestoreautomatic: () => void;
    onrestoremanual: () => void;
  };

  let {
    automaticBackupCount,
    latestAutomaticBackupAtMs,
    isRestoring,
    feedback,
    failed,
    onrestoreautomatic,
    onrestoremanual,
  }: Props = $props();

  /** Formats a native snapshot timestamp without revealing its application-private filename or directory. */
  function backupTime(timestampMs: number): string {
    return new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestampMs);
  }
</script>

<main class="recovery-screen" aria-busy={isRestoring}>
  <section class="recovery-card" aria-labelledby="recovery-title">
    <div class="recovery-mark"><Icon name="database" size={28} /></div>
    <p class="recovery-eyebrow">Local data paused</p>
    <h1 id="recovery-title">Bottie needs to recover its conversation store</h1>
    <p class="recovery-summary">
      SQLite reported that the local database did not pass its integrity check. Conversation and generation actions are
      paused so Bottie does not change the damaged data.
    </p>

    {#if latestAutomaticBackupAtMs !== null}
      <div class="recovery-option preferred">
        <span class="recovery-option-icon"><Icon name="restore" size={19} /></span>
        <span>
          <strong>Latest automatic backup</strong>
          <small>
            Verified snapshot from {backupTime(latestAutomaticBackupAtMs)} · {automaticBackupCount} recovery
            {automaticBackupCount === 1 ? " point" : " points"} available
          </small>
        </span>
        <button type="button" disabled={isRestoring} onclick={onrestoreautomatic}>
          {isRestoring ? "Restoring…" : "Restore latest"}
        </button>
      </div>
    {:else}
      <div class="recovery-option unavailable">
        <span class="recovery-option-icon"><Icon name="database" size={19} /></span>
        <span>
          <strong>No verified automatic backup found</strong>
          <small>Choose a manual Bottie backup to continue.</small>
        </span>
      </div>
    {/if}

    <button class="manual-recovery" type="button" disabled={isRestoring} onclick={onrestoremanual}>
      Choose a manual backup…
    </button>

    <p class="recovery-policy">
      <Icon name="shield" size={15} />
      <span>
        Before replacement, Bottie keeps the damaged database files in app-private storage. Backup locations and local
        database paths never reach the WebView.
      </span>
    </p>
    {#if feedback}
      <p class:error={failed} class="recovery-feedback" role={failed ? "alert" : "status"}>{feedback}</p>
    {/if}
  </section>
</main>
