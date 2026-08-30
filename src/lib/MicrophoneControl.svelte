<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { microphoneFeedback, type MicrophoneStatus } from "$lib/microphone";

  let {
    status,
    disabled,
    onstart,
    onstop,
    ondiscard,
  }: {
    status: MicrophoneStatus;
    disabled: boolean;
    onstart: () => void;
    onstop: () => void;
    ondiscard: () => void;
  } = $props();

  const busy = $derived(status.phase === "starting" || status.phase === "recording");
  const levelPercent = $derived(Math.round(Math.max(0, Math.min(1, status.inputLevel)) * 100));
</script>

<div class="microphone-control">
  {#if status.phase === "recording"}
    <button class="voice-action recording" aria-label="Stop voice capture" onclick={onstop}>
      <span class="stop-square"></span><span>Stop</span>
    </button>
    <span
      class="input-level"
      role="progressbar"
      aria-label="Microphone input level"
      aria-valuemin="0"
      aria-valuemax="100"
      aria-valuenow={levelPercent}
    >
      <span style={`--input-level: ${levelPercent}%`}></span>
    </span>
  {:else}
    <button class="voice-action" aria-label="Record voice locally" disabled={disabled || busy} onclick={onstart}>
      <Icon name="microphone" size={17} />
      <span
        >{status.phase === "captured"
          ? "Record again"
          : status.phase === "starting"
            ? "Waiting…"
            : "Record voice"}</span
      >
    </button>
  {/if}

  {#if status.phase === "captured" || status.phase === "error"}
    <button class="discard-voice" aria-label="Discard voice capture" onclick={ondiscard}>
      <Icon name="trash" size={14} />
    </button>
  {/if}
</div>

<p
  class:error={status.phase === "error"}
  class="microphone-feedback"
  role={status.phase === "error" ? "alert" : "status"}
>
  {microphoneFeedback(status)}
</p>
