<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import AttachmentVisual from "$lib/AttachmentVisual.svelte";
  import MicrophoneControl from "$lib/MicrophoneControl.svelte";
  import { attachmentFailure } from "$lib/attachment";
  import {
    localImageAcquisitionPresentation,
    localImageAvailabilityPresentation,
    type LocalImageAcquisitionStatus,
    type LocalImageAvailabilityMetadata,
  } from "$lib/local-image";
  import type { ImageGenerationExecution } from "$lib/image-generation";
  import { MAX_COMPOSER_ATTACHMENTS, type Attachment, type ProviderStatus } from "$lib/presentation";
  import type { MicrophoneInputDeviceList, MicrophoneStatus } from "$lib/microphone";

  type Props = {
    attachments: Attachment[];
    prompt: string;
    isGenerating: boolean;
    canCompose: boolean;
    canSend: boolean;
    attachmentNote: string;
    providerStatus: ProviderStatus;
    memoryAvailable: boolean;
    memoryEnabled: boolean;
    webAvailable: boolean;
    webEnabled: boolean;
    emailAvailable: boolean;
    emailEnabled: boolean;
    emailBoundaryNote: string;
    emailUnavailableReason: string;
    imageMode: boolean;
    imageExecution: ImageGenerationExecution;
    imageSize: "square" | "landscape" | "portrait";
    imageCount: number;
    imageFeedback: string;
    localImageAvailability: LocalImageAvailabilityMetadata | null;
    localImageAvailabilityFailed: boolean;
    localImageAcquisitionStatus: LocalImageAcquisitionStatus | null;
    localImageAcquisitionFeedback: string;
    microphoneStatus: MicrophoneStatus;
    microphoneAvailable: boolean;
    microphoneWillInterrupt: boolean;
    microphoneAudioAvailable: boolean;
    microphoneAudioUnavailableReason: string;
    microphoneSendAudio: boolean;
    microphoneRetainAudio: boolean;
    microphoneDeviceList: MicrophoneInputDeviceList;
    microphoneDevicesLoaded: boolean;
    microphoneDeviceListFailed: boolean;
    microphoneTranscriptDraftFeedback: string;
    microphoneTranscriptDraftError: boolean;
    onprompt: (prompt: string) => void;
    oninput: () => void;
    onkeydown: (event: KeyboardEvent) => void;
    onsend: () => void;
    onadd: () => void;
    onfiles: (event: Event) => void;
    onremove: (id: string) => void;
    ontogglememory: () => void;
    ontoggleweb: () => void;
    ontoggleemail: () => void;
    ontoggleimage: () => void;
    onimageexecution: (execution: ImageGenerationExecution) => void;
    onimagesize: (size: "square" | "landscape" | "portrait") => void;
    onimagecount: (count: number) => void;
    oninstalllocalimage: () => void;
    oncancellocalimage: () => void;
    onstartmicrophone: () => void;
    onstopmicrophone: () => void;
    ondiscardmicrophone: () => void;
    oncorrectmicrophone: (turnIndex: number, text: string) => void;
    ontogglesendmicrophoneaudio: () => void;
    ontoggleretainmicrophoneaudio: () => void;
    onloadmicrophonedevices: () => void;
    onselectmicrophonedevice: (token: string) => void;
    onusemicrophonetranscript: () => void;
    oncomposerready: (element: HTMLTextAreaElement) => void;
    onattachmentinputready: (element: HTMLInputElement) => void;
  };

  let {
    attachments,
    prompt,
    isGenerating,
    canCompose,
    canSend,
    attachmentNote,
    providerStatus,
    memoryAvailable,
    memoryEnabled,
    webAvailable,
    webEnabled,
    emailAvailable,
    emailEnabled,
    emailBoundaryNote,
    emailUnavailableReason,
    imageMode,
    imageExecution,
    imageSize,
    imageCount,
    imageFeedback,
    localImageAvailability,
    localImageAvailabilityFailed,
    localImageAcquisitionStatus,
    localImageAcquisitionFeedback,
    microphoneStatus,
    microphoneAvailable,
    microphoneWillInterrupt,
    microphoneAudioAvailable,
    microphoneAudioUnavailableReason,
    microphoneSendAudio,
    microphoneRetainAudio,
    microphoneDeviceList,
    microphoneDevicesLoaded,
    microphoneDeviceListFailed,
    microphoneTranscriptDraftFeedback,
    microphoneTranscriptDraftError,
    onprompt,
    oninput,
    onkeydown,
    onsend,
    onadd,
    onfiles,
    onremove,
    ontogglememory,
    ontoggleweb,
    ontoggleemail,
    ontoggleimage,
    onimageexecution,
    onimagesize,
    onimagecount,
    oninstalllocalimage,
    oncancellocalimage,
    onstartmicrophone,
    onstopmicrophone,
    ondiscardmicrophone,
    oncorrectmicrophone,
    ontogglesendmicrophoneaudio,
    ontoggleretainmicrophoneaudio,
    onloadmicrophonedevices,
    onselectmicrophonedevice,
    onusemicrophonetranscript,
    oncomposerready,
    onattachmentinputready,
  }: Props = $props();
  let composer: HTMLTextAreaElement;
  let attachmentInput: HTMLInputElement;

  $effect(() => {
    if (composer) oncomposerready(composer);
    if (attachmentInput) onattachmentinputready(attachmentInput);
  });
</script>

<footer class="composer-zone">
  <div class="composer-shell" class:busy={isGenerating}>
    {#if attachments.length > 0}
      <div class="composer-attachments">
        {#each attachments.slice(0, MAX_COMPOSER_ATTACHMENTS) as attachment (attachment.id)}
          {@const failure = attachmentFailure(attachment)}
          <div
            class:failed={Boolean(failure)}
            class="attachment-chip"
            title={failure ? `${failure.title}. ${failure.detail}` : undefined}
          >
            <AttachmentVisual {attachment} className="chip-icon" iconSize={14} />
            <span>{attachment.name}</span>
            {#if failure}
              <span class="visually-hidden">{failure.title}. {failure.detail}</span>
            {/if}
            <button aria-label={`Remove ${attachment.name}`} onclick={() => onremove(attachment.id)}>
              <Icon name="x" size={13} />
            </button>
          </div>
        {/each}
        {#if attachments.length > MAX_COMPOSER_ATTACHMENTS}
          <span class="more-files">+{attachments.length - MAX_COMPOSER_ATTACHMENTS}</span>
        {/if}
      </div>
    {/if}

    <textarea
      bind:this={composer}
      value={prompt}
      oninput={(event) => {
        onprompt(event.currentTarget.value);
        oninput();
      }}
      {onkeydown}
      rows="1"
      disabled={!canCompose && !isGenerating}
      placeholder={imageMode
        ? "Describe the image to generate…"
        : providerStatus === "available"
          ? "Message the selected model…"
          : "Connect a provider to send a message"}
      aria-describedby={`composer-guidance${emailEnabled || emailUnavailableReason ? " composer-email-guidance" : ""}`}
      aria-label="Message bottie"></textarea>

    <div class="composer-toolbar">
      <div class="composer-tools">
        <input
          class="visually-hidden"
          bind:this={attachmentInput}
          onchange={onfiles}
          type="file"
          multiple
          tabindex="-1"
        />
        <button aria-label="Attach files" disabled={imageMode || isGenerating} onclick={onadd}>
          <Icon name="paperclip" size={18} />
        </button>
        <button
          class="tool-toggle"
          aria-label={imageMode ? "Use text chat" : "Generate an image"}
          aria-pressed={imageMode}
          disabled={isGenerating}
          onclick={ontoggleimage}
        >
          <Icon name="image" size={17} /><span>Image</span>
        </button>
        <button
          class="tool-toggle"
          aria-label={memoryAvailable
            ? memoryEnabled
              ? "Disable memory tools"
              : "Enable memory tools"
            : "Memory tools require a supported tool-capable model"}
          aria-pressed={memoryAvailable && memoryEnabled}
          disabled={!memoryAvailable || isGenerating}
          onclick={ontogglememory}
        >
          <Icon name="brain" size={17} /><span>Memory</span>
        </button>
        <button
          class="tool-toggle"
          aria-label={webAvailable
            ? webEnabled
              ? "Disable web search"
              : "Enable web search"
            : "Web search requires a supported tool-capable model"}
          aria-pressed={webAvailable && webEnabled}
          disabled={!webAvailable || isGenerating}
          onclick={ontoggleweb}
        >
          <Icon name="globe" size={17} /><span>Web</span>
        </button>
        <button
          class="tool-toggle"
          aria-label={emailAvailable
            ? emailEnabled
              ? "Disable email tools"
              : "Enable email tools"
            : emailUnavailableReason}
          title={!emailAvailable ? emailUnavailableReason : undefined}
          aria-pressed={emailAvailable && emailEnabled}
          disabled={!emailAvailable || isGenerating}
          onclick={ontoggleemail}
        >
          <Icon name="mail" size={17} /><span>Email</span>
        </button>
      </div>

      <button
        class="send-button"
        class:enabled={(prompt.trim().length > 0 && canSend) || isGenerating}
        disabled={(!prompt.trim() || !canSend) && !isGenerating}
        aria-label={isGenerating ? "Stop generating" : imageMode ? "Generate image" : "Send message"}
        onclick={onsend}
      >
        {#if isGenerating}
          <span class="stop-square"></span>
        {:else}
          <Icon name="arrow-up" size={19} strokeWidth={2.2} />
        {/if}
      </button>
    </div>
    {#if imageMode}
      {@const localImage = localImageAvailabilityPresentation(localImageAvailability, localImageAvailabilityFailed)}
      {@const localImageReady = localImageAvailability?.availability === "ready"}
      {@const acquisition = localImageAcquisitionStatus
        ? localImageAcquisitionPresentation(localImageAcquisitionStatus)
        : null}
      <div class="image-generation-options" aria-label="Image generation options">
        <label>
          <span>Execution</span>
          <select
            aria-label="Execution"
            value={imageExecution}
            disabled={isGenerating}
            onchange={(event) => onimageexecution(event.currentTarget.value as ImageGenerationExecution)}
          >
            <option value="cloud">Cloud</option>
            <option value="local" disabled={!localImageReady}
              >Local 2512{localImageReady ? "" : " · unavailable"}</option
            >
          </select>
        </label>
        {#if imageExecution === "cloud"}
          <label>
            <span>Size</span>
            <select
              value={imageSize}
              disabled={isGenerating}
              onchange={(event) => onimagesize(event.currentTarget.value as typeof imageSize)}
            >
              <option value="square">Square · 2048×2048</option>
              <option value="landscape">Landscape · 2688×1536</option>
              <option value="portrait">Portrait · 1536×2688</option>
            </select>
          </label>
          <label>
            <span>Images</span>
            <select
              value={imageCount}
              disabled={isGenerating}
              onchange={(event) => onimagecount(Number(event.currentTarget.value))}
            >
              {#each [1, 2, 3, 4, 5, 6] as count}<option value={count}>{count}</option>{/each}
            </select>
          </label>
          <span class="image-execution"><strong>Cloud</strong> · <code>qwen-image-2.0-2026-03-03</code></span>
        {:else}
          <span class="image-execution"><strong>Local</strong> · 512×512 · one image</span>
        {/if}
      </div>
      {#if imageExecution === "cloud"}
        <p class="image-delivery-note">
          Your image prompt is sent to Alibaba Model Studio and may incur provider charges. Rust downloads each
          temporary result immediately, validates it, and stores only app-private PNG bytes.
        </p>
      {:else}
        <p class="image-delivery-note">
          Your prompt and generated bytes stay on this device. Rust uses the verified local worker and exact
          <code>Qwen/Qwen-Image-2512</code> package, then validates and stores the PNG privately.
        </p>
      {/if}
      <section class="local-image-availability" aria-label="Local image availability">
        <div class="local-image-heading">
          <strong>Local 2512</strong>
          <span class:ready={localImage.state === "ready"}>{localImage.label}</span>
        </div>
        {#if localImageAvailability}
          <code>{localImageAvailability.modelId}</code>
          <span>
            <code>{localImageAvailability.packageId}</code> · MLX-Gen runtime
            <code>{localImageAvailability.runtimeId}</code>
          </span>
          <span>{localImageAvailability.license} · revision <code>{localImageAvailability.sourceRevision}</code></span>
        {/if}
        <small>{localImage.detail}. No automatic download or cloud fallback.</small>
        {#if acquisition && (acquisition.action !== "none" || acquisition.active)}
          <div class="local-image-acquisition" aria-label="Local image model installation">
            <p>
              Download the disclosed immutable package from Hugging Face into Bottie’s app-owned cache. Network access
              is used only for acquisition; generation remains offline.
            </p>
            {#if acquisition.active || acquisition.percent > 0}
              <progress aria-label="Local model download progress" max="100" value={acquisition.percent}
                >{acquisition.percent}%</progress
              >
            {/if}
            <span>{acquisition.detail}</span>
            {#if acquisition.action === "cancel"}
              <strong>{acquisition.label}</strong>
              <button type="button" onclick={oncancellocalimage}>Cancel download</button>
            {:else if ["install", "resume", "retry"].includes(acquisition.action)}
              <button type="button" disabled={isGenerating} onclick={oninstalllocalimage}>{acquisition.label}</button>
            {:else}
              <strong>{acquisition.label}</strong>
            {/if}
          </div>
        {/if}
        {#if localImageAcquisitionFeedback}
          <span class="local-image-acquisition-feedback" role="status">{localImageAcquisitionFeedback}</span>
        {/if}
      </section>
      {#if imageFeedback}<p class="image-generation-feedback" role="status">{imageFeedback}</p>{/if}
    {/if}
    <MicrophoneControl
      status={microphoneStatus}
      disabled={!microphoneAvailable}
      willInterrupt={microphoneWillInterrupt}
      audioAvailable={microphoneAudioAvailable}
      audioUnavailableReason={microphoneAudioUnavailableReason}
      sendAudio={microphoneSendAudio}
      retainAudio={microphoneRetainAudio}
      deviceList={microphoneDeviceList}
      devicesLoaded={microphoneDevicesLoaded}
      deviceListFailed={microphoneDeviceListFailed}
      onstart={onstartmicrophone}
      onstop={onstopmicrophone}
      ondiscard={ondiscardmicrophone}
      oncorrect={oncorrectmicrophone}
      ontogglesendaudio={ontogglesendmicrophoneaudio}
      ontoggleretainaudio={ontoggleretainmicrophoneaudio}
      onloaddevices={onloadmicrophonedevices}
      onselectdevice={onselectmicrophonedevice}
      onusetext={onusemicrophonetranscript}
      transcriptDraftFeedback={microphoneTranscriptDraftFeedback}
      transcriptDraftError={microphoneTranscriptDraftError}
    />
  </div>
  <p id="composer-guidance" class="composer-note" aria-live="polite">
    {attachmentNote}
  </p>
  {#if emailEnabled}
    <p id="composer-email-guidance" class="email-boundary-note">{emailBoundaryNote}</p>
  {:else if emailUnavailableReason}
    <p id="composer-email-guidance" class="email-boundary-note" role="status">{emailUnavailableReason}</p>
  {/if}
</footer>
