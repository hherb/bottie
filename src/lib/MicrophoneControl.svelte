<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import {
    formatMicrophoneDuration,
    MAX_TRANSCRIPT_TURN_BYTES,
    microphoneFeedback,
    normalizeTranscriptCorrection,
    type MicrophoneStatus,
  } from "$lib/microphone";

  let {
    status,
    disabled,
    onstart,
    onstop,
    ondiscard,
    oncorrect,
  }: {
    status: MicrophoneStatus;
    disabled: boolean;
    onstart: () => void;
    onstop: () => void;
    ondiscard: () => void;
    oncorrect: (turnIndex: number, text: string) => void;
  } = $props();

  const busy = $derived(
    status.phase === "starting" ||
      status.phase === "recording" ||
      status.transcriptionPhase === "preparing_model" ||
      status.transcriptionPhase === "transcribing",
  );
  const hasError = $derived(status.phase === "error" || status.transcriptionPhase === "error");
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

<p class:error={hasError} class="microphone-feedback" role={hasError ? "alert" : "status"}>
  {microphoneFeedback(status)}
</p>

{#if status.transcriptSegments.length > 0}
  <ol class="voice-transcript" aria-label="Local voice transcript" aria-live="polite">
    {#each status.transcriptSegments as segment, turnIndex}
      <li aria-label={`Voice turn ${turnIndex + 1}`}>
        <span class="transcript-turn">Turn {turnIndex + 1}</span>
        <span class="transcript-time"
          >{formatMicrophoneDuration(segment.startMs)}–{formatMicrophoneDuration(segment.endMs)}</span
        >
        {#if status.transcriptionPhase === "ready" && segment.isFinal}
          <form
            class="transcript-correction"
            onsubmit={(event) => {
              event.preventDefault();
              const input = event.currentTarget.elements.namedItem("correction") as HTMLInputElement;
              const text = normalizeTranscriptCorrection(input.value);
              input.setCustomValidity(text ? "" : "Keep this correction within 512 UTF-8 bytes.");
              if (text) oncorrect(turnIndex, text);
              else input.reportValidity();
            }}
          >
            <input
              name="correction"
              aria-label={`Correct voice turn ${turnIndex + 1}`}
              value={segment.text}
              maxlength={MAX_TRANSCRIPT_TURN_BYTES}
              required
              oninput={(event) => event.currentTarget.setCustomValidity("")}
            />
            <button aria-label={`Save correction for voice turn ${turnIndex + 1}`} type="submit">Save</button>
          </form>
          {#if segment.isCorrected}<span class="corrected-transcript">Corrected</span>{/if}
        {:else}
          <span>{segment.text}</span>
          <span class="partial-transcript">Partial</span>
        {/if}
      </li>
    {/each}
  </ol>
{/if}
