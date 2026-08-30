<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import AttachmentVisual from "$lib/AttachmentVisual.svelte";
  import MicrophoneControl from "$lib/MicrophoneControl.svelte";
  import { attachmentFailure } from "$lib/attachment";
  import { MAX_COMPOSER_ATTACHMENTS, type Attachment, type ProviderStatus } from "$lib/presentation";
  import type { MicrophoneStatus } from "$lib/microphone";

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
    microphoneStatus: MicrophoneStatus;
    microphoneAvailable: boolean;
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
    onstartmicrophone: () => void;
    onstopmicrophone: () => void;
    ondiscardmicrophone: () => void;
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
    microphoneStatus,
    microphoneAvailable,
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
    onstartmicrophone,
    onstopmicrophone,
    ondiscardmicrophone,
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
      placeholder={providerStatus === "available"
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
        <button aria-label="Attach files" onclick={onadd}>
          <Icon name="paperclip" size={18} />
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
        aria-label={isGenerating ? "Stop generating" : "Send message"}
        onclick={onsend}
      >
        {#if isGenerating}
          <span class="stop-square"></span>
        {:else}
          <Icon name="arrow-up" size={19} strokeWidth={2.2} />
        {/if}
      </button>
    </div>
    <MicrophoneControl
      status={microphoneStatus}
      disabled={!microphoneAvailable || isGenerating}
      onstart={onstartmicrophone}
      onstop={onstopmicrophone}
      ondiscard={ondiscardmicrophone}
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
