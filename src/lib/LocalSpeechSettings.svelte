<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { speechFeedback, type SpeechSettingsState } from "$lib/speech";

  let { speech, disabled }: { speech: SpeechSettingsState; disabled: boolean } = $props();
</script>

<section class="provider-setting local-speech-setting" aria-labelledby="local-speech-settings-title">
  <div class="provider-setting-heading">
    <span>
      <strong id="local-speech-settings-title">Local speech voice</strong>
      <small>Used for explicit Play response aloud actions</small>
    </span>
    <span class="local-badge"><Icon name="speaker" size={12} /> On device</span>
  </div>
  <label for="local-speech-voice">Voice</label>
  <select
    id="local-speech-voice"
    aria-label="Local playback voice"
    value={speech.status.selectedVoiceId ?? ""}
    disabled={disabled || !speech.available || speech.voices.length === 0 || speech.status.phase === "speaking"}
    onchange={(event) => void speech.selectVoice(event.currentTarget.value)}
  >
    {#if speech.voices.length === 0}<option value="">No voices available</option>{/if}
    {#each speech.voices as voice (voice.id)}
      <option value={voice.id}>{voice.name} · {voice.language || "Unknown language"}</option>
    {/each}
  </select>
  <p class:error={speech.status.phase === "error"} class="credential-status" role="status">
    {speechFeedback(speech.status, speech.voices.length)} · Saved automatically on this device.
  </p>
  <p class="local-speech-boundary">
    Voice identity stays native. If the saved voice is unavailable after restart, Bottie uses the default local voice.
  </p>
</section>

<style>
  .local-speech-setting select {
    width: 100%;
  }
  .local-speech-boundary {
    margin: 8px 0 0;
    color: #6f8b87;
    font-size: 8px;
    line-height: 1.5;
  }
</style>
