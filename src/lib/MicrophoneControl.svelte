<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import {
    formatMicrophoneDuration,
    MAX_TRANSCRIPT_TURN_BYTES,
    microphoneFeedback,
    microphoneLatencyFeedback,
    normalizeTranscriptCorrection,
    type MicrophoneInputDeviceList,
    type MicrophoneStatus,
  } from "$lib/microphone";

  let {
    status,
    disabled,
    willInterrupt,
    audioAvailable,
    audioUnavailableReason,
    sendAudio,
    retainAudio,
    deviceList,
    devicesLoaded,
    deviceListFailed,
    onstart,
    onstop,
    ondiscard,
    oncorrect,
    ontogglesendaudio,
    ontoggleretainaudio,
    onloaddevices,
    onselectdevice,
  }: {
    status: MicrophoneStatus;
    disabled: boolean;
    willInterrupt: boolean;
    audioAvailable: boolean;
    audioUnavailableReason: string;
    sendAudio: boolean;
    retainAudio: boolean;
    deviceList: MicrophoneInputDeviceList;
    devicesLoaded: boolean;
    deviceListFailed: boolean;
    onstart: () => void;
    onstop: () => void;
    ondiscard: () => void;
    oncorrect: (turnIndex: number, text: string) => void;
    ontogglesendaudio: () => void;
    ontoggleretainaudio: () => void;
    onloaddevices: () => void;
    onselectdevice: (token: string) => void;
  } = $props();

  const busy = $derived(
    status.phase === "starting" ||
      status.phase === "recording" ||
      status.transcriptionPhase === "preparing_model" ||
      status.transcriptionPhase === "transcribing",
  );
  const hasError = $derived(status.phase === "error" || status.transcriptionPhase === "error");
  const levelPercent = $derived(Math.round(Math.max(0, Math.min(1, status.inputLevel)) * 100));
  const latencyFeedback = $derived(microphoneLatencyFeedback(status));
  const availableDeviceCount = $derived(deviceList.devices.filter((device) => !device.isSystemDefault).length);
  const deviceCountFeedback = $derived(
    `${availableDeviceCount} ${availableDeviceCount === 1 ? "microphone" : "microphones"} available · ` +
      "selection stays only for this app session",
  );
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
    <button
      class="voice-action"
      aria-label={willInterrupt ? "Interrupt Bottie and record voice locally" : "Record voice locally"}
      disabled={disabled || busy}
      onclick={onstart}
    >
      <Icon name="microphone" size={17} />
      <span
        >{status.phase === "captured"
          ? "Record again"
          : status.phase === "starting"
            ? "Waiting…"
            : willInterrupt
              ? "Interrupt & record"
              : "Record voice"}</span
      >
    </button>
  {/if}

  {#if status.phase === "captured" || status.phase === "error"}
    <button class="discard-voice" aria-label="Discard voice capture" onclick={ondiscard}>
      <Icon name="trash" size={14} />
    </button>
  {/if}

  <div class="microphone-device-picker">
    {#if devicesLoaded}
      <select
        aria-label="Microphone input"
        value={deviceList.selectedToken}
        disabled={disabled || busy}
        onchange={(event) => onselectdevice(event.currentTarget.value)}
      >
        {#if !deviceList.selectionAvailable}
          <option value={deviceList.selectedToken} disabled>Unavailable microphone</option>
        {/if}
        {#each deviceList.devices as device (device.token)}
          <option value={device.token}>{device.label}</option>
        {/each}
      </select>
      <button
        class="refresh-microphones"
        aria-label="Refresh microphone choices"
        disabled={disabled || busy}
        onclick={onloaddevices}>Refresh</button
      >
    {:else}
      <button
        class="choose-microphone"
        aria-label="Choose microphone input"
        disabled={disabled || busy}
        onclick={onloaddevices}>Choose microphone</button
      >
    {/if}
  </div>
</div>

<p class:error={hasError} class="microphone-feedback" role={hasError ? "alert" : "status"}>
  {microphoneFeedback(status)}
</p>

{#if deviceListFailed}
  <p class="microphone-device-note error" role="alert">
    Microphone choices could not be refreshed. Your current session selection is unchanged.
  </p>
{:else if devicesLoaded && !deviceList.selectionAvailable}
  <p class="microphone-device-note error" role="alert">
    Selected microphone is no longer available. Choose another microphone before recording.
  </p>
{:else if devicesLoaded && availableDeviceCount === 0}
  <p class="microphone-device-note" role="status">
    No microphones are currently available. System default will be checked when you Record.
  </p>
{:else if devicesLoaded}
  <p class="microphone-device-note" role="status">{deviceCountFeedback}</p>
{/if}

{#if latencyFeedback}
  <p class="voice-latency" aria-label="Local voice timing">{latencyFeedback}</p>
{/if}

{#if status.phase === "captured"}
  <div class="audio-delivery" aria-label="Recording delivery choices">
    <button
      class="audio-choice"
      aria-label={sendAudio ? "Stop sending recording with the next message" : "Send recording with the next message"}
      aria-pressed={sendAudio}
      disabled={disabled || (!audioAvailable && !sendAudio)}
      title={!audioAvailable ? audioUnavailableReason : undefined}
      onclick={ontogglesendaudio}
    >
      Send recording
    </button>
    <button
      class="audio-choice"
      aria-label={retainAudio
        ? "Do not retain recording locally with the message"
        : "Retain recording locally with the message"}
      aria-pressed={retainAudio}
      {disabled}
      onclick={ontoggleretainaudio}
    >
      Retain locally
    </button>
  </div>
  <p class="audio-delivery-note" role="status">
    {sendAudio && !audioAvailable
      ? `${audioUnavailableReason} Turn off Send recording or choose an audio-capable model.`
      : sendAudio && retainAudio
        ? "Recording will be sent and retained as a local WAV attachment."
        : sendAudio
          ? "Recording will be sent once and removed from session memory after acceptance."
          : retainAudio
            ? "Recording will be retained as a local WAV attachment and will not be sent."
            : audioAvailable
              ? "Recording stays native-only until you explicitly choose delivery or retention."
              : `${audioUnavailableReason} You can still retain it locally.`}
  </p>
{/if}

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
