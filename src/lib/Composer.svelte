<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import AttachmentVisual from "$lib/AttachmentVisual.svelte";
  import { attachmentFailure } from "$lib/attachment";
  import { MAX_COMPOSER_ATTACHMENTS, type Attachment, type ProviderStatus } from "$lib/presentation";

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
    onprompt: (prompt: string) => void;
    oninput: () => void;
    onkeydown: (event: KeyboardEvent) => void;
    onsend: () => void;
    onadd: () => void;
    onfiles: (event: Event) => void;
    onremove: (id: string) => void;
    ontogglememory: () => void;
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
    onprompt,
    oninput,
    onkeydown,
    onsend,
    onadd,
    onfiles,
    onremove,
    ontogglememory,
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
            : "Memory tools require a tool-capable Ollama or OpenAI-compatible model"}
          aria-pressed={memoryAvailable && memoryEnabled}
          disabled={!memoryAvailable || isGenerating}
          onclick={ontogglememory}
        >
          <Icon name="brain" size={17} /><span>Memory</span>
        </button>
        <button class="tool-toggle" aria-label="Web search is not available yet" disabled>
          <Icon name="globe" size={17} /><span>Web</span>
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
  </div>
  <p class="composer-note">
    {attachmentNote}
  </p>
</footer>
