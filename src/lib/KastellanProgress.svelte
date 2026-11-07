<!--
  The live part of a running Kastellan response: the task's progress lines as they arrive, a Stop button, and a field
  for an operator correction the planner reads on its next turn. Shown only while the task runs; the saved reply
  replaces it.
-->
<script lang="ts">
  import Icon from "./Icon.svelte";
  import type { KastellanLive } from "./kastellan";

  type Props = {
    live: KastellanLive;
    onstop: (runId: string) => void;
    /** Resolves to whether the correction was accepted, so the field is cleared only then. */
    oncorrect: (runId: string, text: string) => Promise<boolean>;
  };

  let { live, onstop, oncorrect }: Props = $props();
  let correction = $state("");
  let sending = $state(false);
  let stopping = $state(false);

  async function submitCorrection(event: SubmitEvent) {
    event.preventDefault();
    const text = correction.trim();
    if (!text || sending) return;
    sending = true;
    if (await oncorrect(live.runId, text)) correction = "";
    sending = false;
  }
</script>

<div class="kastellan-live" aria-live="polite">
  {#if live.progress.length}
    <ol class="kastellan-progress">
      {#each live.progress as line, index (index)}<li>{line}</li>{/each}
    </ol>
  {:else}
    <p class="kastellan-progress-empty">Waiting for Kastellan's first step…</p>
  {/if}
  <form class="kastellan-controls" onsubmit={(event) => void submitCorrection(event)}>
    <input
      type="text"
      aria-label="Correction for Kastellan"
      placeholder="Correct the task while it runs…"
      bind:value={correction}
      disabled={sending}
    />
    <button type="submit" disabled={sending || !correction.trim()}>Correct</button>
    <button
      type="button"
      class="kastellan-stop"
      aria-label="Stop Kastellan task"
      disabled={stopping}
      onclick={() => {
        stopping = true;
        onstop(live.runId);
      }}><Icon name="stop" size={13} /><span>Stop</span></button
    >
  </form>
</div>
