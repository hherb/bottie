<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import ImageComposerOptions from "$lib/ImageComposerOptions.svelte";
  import ComposerTabs from "$lib/ComposerTabs.svelte";
  import AttachmentVisual from "$lib/AttachmentVisual.svelte";
  import MicrophoneControl from "$lib/MicrophoneControl.svelte";
  import { attachmentFailure } from "$lib/attachment";
  import { type LocalImageAcquisitionStatus, type LocalImageAvailabilityMetadata } from "$lib/local-image";
  import type { ImageGenerationExecution, ImageGenerationSize, ImageGenerationResolution } from "$lib/image-generation";
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
    imageSize: ImageGenerationSize;
    imageResolution?: ImageGenerationResolution;
    modeSwitchDisabled?: boolean;
    imageCount: number;
    generatedImageSourceCount: number;
    imageFeedback: string;
    localImageAvailability: LocalImageAvailabilityMetadata | null;
    localImageAvailabilityFailed: boolean;
    localImageAcquisitionStatus: LocalImageAcquisitionStatus | null;
    localImageAcquisitionFeedback: string;
    localImageWorkerImporting: boolean;
    localImageWorkerImportFeedback: string;
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
    onimagesize: (size: ImageGenerationSize) => void;
    onimageresolution?: (resolution: ImageGenerationResolution) => void;
    onimagecount: (count: number) => void;
    oninstalllocalimage: () => void;
    oncancellocalimage: () => void;
    onimportlocalimageworker: () => void;
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
    imageResolution,
    modeSwitchDisabled = false,
    imageCount,
    generatedImageSourceCount,
    imageFeedback,
    localImageAvailability,
    localImageAvailabilityFailed,
    localImageAcquisitionStatus,
    localImageAcquisitionFeedback,
    localImageWorkerImporting,
    localImageWorkerImportFeedback,
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
    onimageresolution = () => {},
    onimagecount,
    oninstalllocalimage,
    oncancellocalimage,
    onimportlocalimageworker,
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
    <ComposerTabs
      {imageMode}
      disabled={isGenerating || modeSwitchDisabled}
      onselect={(image) => {
        if (imageMode !== image) ontoggleimage();
      }}
    />
    <div id="composer-panel" role="tabpanel" tabindex="0" aria-labelledby={imageMode ? "image-tab" : "chat-tab"}>
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
              <button
                aria-label={`Remove ${attachment.name}`}
                disabled={!canCompose || isGenerating}
                onclick={() => onremove(attachment.id)}
              >
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
        aria-describedby={`composer-guidance${!imageMode && (emailEnabled || emailUnavailableReason) ? " composer-email-guidance" : ""}`}
        aria-label={imageMode ? "Image prompt" : "Message bottie"}></textarea>

      <div class="composer-toolbar">
        <div class="composer-tools">
          <input
            class="visually-hidden"
            bind:this={attachmentInput}
            onchange={onfiles}
            type="file"
            multiple
            accept={imageMode ? "image/png,image/jpeg" : undefined}
            tabindex="-1"
          />
          <button
            aria-label={imageMode
              ? imageExecution === "cloud"
                ? attachments.length + generatedImageSourceCount >= 3
                  ? "Reference image limit reached"
                  : "Attach reference images"
                : "Reference-image editing is available only with Cloud execution"
              : "Attach files"}
            disabled={!canCompose ||
              (imageMode && (imageExecution !== "cloud" || attachments.length + generatedImageSourceCount >= 3)) ||
              isGenerating}
            onclick={onadd}
          >
            <Icon name="paperclip" size={18} />
          </button>
          {#if !imageMode}
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
          {/if}
        </div>

        <button
          class="send-button"
          class:enabled={(prompt.trim().length > 0 && canSend) || isGenerating}
          disabled={(!prompt.trim() || !canSend) && !isGenerating}
          aria-label={isGenerating
            ? "Stop generating"
            : imageMode
              ? attachments.length + generatedImageSourceCount > 0
                ? "Edit image"
                : "Generate image"
              : "Send message"}
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
        <ImageComposerOptions
          {attachments}
          isGenerating={isGenerating || modeSwitchDisabled}
          {imageExecution}
          {imageSize}
          {imageResolution}
          {imageCount}
          {generatedImageSourceCount}
          {imageFeedback}
          {localImageAvailability}
          {localImageAvailabilityFailed}
          {localImageAcquisitionStatus}
          {localImageAcquisitionFeedback}
          {localImageWorkerImporting}
          {localImageWorkerImportFeedback}
          {onimageexecution}
          {onimagesize}
          {onimageresolution}
          {onimagecount}
          {oninstalllocalimage}
          {oncancellocalimage}
          {onimportlocalimageworker}
        />
      {:else}
        <details class="composer-voice" open={microphoneStatus.phase !== "idle"}>
          <summary>Voice input</summary>
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
        </details>
      {/if}
    </div>
  </div>
  <p id="composer-guidance" class="composer-note" aria-live="polite">
    {attachmentNote}
  </p>
  {#if !imageMode && emailEnabled}
    <p id="composer-email-guidance" class="email-boundary-note">{emailBoundaryNote}</p>
  {:else if !imageMode && emailUnavailableReason}
    <p id="composer-email-guidance" class="email-boundary-note" role="status">{emailUnavailableReason}</p>
  {/if}
</footer>
