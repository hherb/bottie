<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import type { ModelInfo } from "$lib/inference";
  import { attachmentStatusLabel } from "$lib/attachment";
  import { attachmentDeliveryLabel } from "$lib/chat";
  import type { Attachment, MessageAttachment, ProviderStatus } from "$lib/presentation";

  type Props = {
    open: boolean;
    attachments: Attachment[];
    conversationAttachments: MessageAttachment[];
    selectedModel: ModelInfo | undefined;
    selectedProviderEndpoint: string;
    providerStatus: ProviderStatus;
    isLocalRoute: boolean;
    isAddingAttachments: boolean;
    attachmentFeedback: string | null;
    attachmentFailed: boolean;
    attachmentActionsDisabled: boolean;
    onclose: () => void;
    onadd: () => void;
    onremove: (id: string) => void;
    onremoveassociated: (messageId: string, attachmentId: string) => void;
  };

  let {
    open,
    attachments,
    conversationAttachments,
    selectedModel,
    selectedProviderEndpoint,
    providerStatus,
    isLocalRoute,
    isAddingAttachments,
    attachmentFeedback,
    attachmentFailed,
    attachmentActionsDisabled,
    onclose,
    onadd,
    onremove,
    onremoveassociated,
  }: Props = $props();
</script>

<aside class:closed={!open} class="context-panel" aria-label="Conversation context">
  <div class="context-header">
    <div>
      <span class="eyebrow">This conversation</span>
      <h2>Context</h2>
    </div>
    <button class="icon-button" aria-label="Close context panel" onclick={onclose}>
      <Icon name="x" size={18} />
    </button>
  </div>

  <section class="context-section">
    <div class="section-heading">
      <h3>Attachments <span>{attachments.length + conversationAttachments.length}</span></h3>
      <button disabled={isAddingAttachments} onclick={onadd}>{isAddingAttachments ? "Adding…" : "Add"}</button>
    </div>
    <div class="attachment-list">
      {#each attachments as attachment (attachment.id)}
        <div class="attachment-row">
          <span class:image={attachment.kind === "image"} class="attachment-icon">
            <Icon name={attachment.kind} size={18} />
          </span>
          <span class="attachment-copy">
            <strong>{attachment.name}</strong>
            <small
              >{attachment.size} · {attachmentStatusLabel(attachment.normalization, attachment.extraction)} ·
              {attachmentDeliveryLabel(attachment, selectedModel)}</small
            >
          </span>
          <button aria-label={`Remove ${attachment.name}`} onclick={() => onremove(attachment.id)}>
            <Icon name="x" size={15} />
          </button>
        </div>
      {/each}
      {#each conversationAttachments as association (`${association.messageId}:${association.attachment.id}`)}
        <div class="attachment-row retained-association">
          <span class:image={association.attachment.kind === "image"} class="attachment-icon">
            <Icon name={association.attachment.kind} size={18} />
          </span>
          <span class="attachment-copy">
            <strong>{association.attachment.name}</strong>
            <small>
              {association.attachment.size} ·
              {attachmentStatusLabel(association.attachment.normalization, association.attachment.extraction)} ·
              {attachmentDeliveryLabel(association.attachment, selectedModel)}
            </small>
          </span>
          <button
            aria-label={`Remove ${association.attachment.name} from message`}
            disabled={attachmentActionsDisabled}
            onclick={() => onremoveassociated(association.messageId, association.attachment.id)}
          >
            <Icon name="x" size={15} />
          </button>
        </div>
      {/each}
      {#if attachments.length === 0 && conversationAttachments.length === 0}
        <button class="empty-attachments" onclick={onadd}>
          <Icon name="paperclip" size={18} />
          <span><strong>Add context</strong><small>Images, documents, or text files</small></span>
        </button>
      {/if}
    </div>
    {#if attachmentFeedback}
      <p class:error={attachmentFailed} class="attachment-feedback" role="status">{attachmentFeedback}</p>
    {/if}
  </section>

  <section class="context-section memory-section">
    <div class="section-heading">
      <h3>Preview memories <span>3 fixtures</span></h3>
      <button disabled>Not active</button>
    </div>
    <div class="memory-card cyan">
      <div class="memory-meta"><Icon name="brain" size={14} /> Architecture discussion <span>92%</span></div>
      <p>Keep secrets, storage, provider calls, and tool execution inside the Rust core.</p>
      <small>Today · Bottie architecture</small>
    </div>
    <div class="memory-card violet">
      <div class="memory-meta"><Icon name="brain" size={14} /> Search design <span>86%</span></div>
      <p>Combine SQLite full-text and vector results with reciprocal-rank fusion.</p>
      <small>Yesterday · SQLite search notes</small>
    </div>
    <div class="memory-card amber">
      <div class="memory-meta"><Icon name="brain" size={14} /> Interface preference <span>79%</span></div>
      <p>Tool activity should be visible, calm, and expandable when details matter.</p>
      <small>Today · Bottie architecture</small>
    </div>
  </section>

  <section class="context-section route-section">
    <div class="section-heading"><h3>Privacy route</h3></div>
    <div class="route-card">
      <div class="route-line">
        <span class="route-node device"><Icon name="shield" size={15} /></span>
        <span class="route-track"></span>
        <span class="route-node model"><span class="tiny-core"></span></span>
      </div>
      <div class="route-labels">
        <span
          ><strong>This Mac</strong><small>{isLocalRoute ? "Conversation stays local" : "Prompt leaves device"}</small
          ></span
        >
        <span>
          <strong>{selectedModel?.providerName ?? "Selected provider"}</strong>
          <small>{selectedProviderEndpoint}</small>
        </span>
      </div>
      <div class:cloud={!isLocalRoute} class:offline={providerStatus !== "available"} class="route-status">
        <span></span>
        {providerStatus === "available"
          ? isLocalRoute
            ? "Connected over loopback"
            : "Cloud transmission enabled"
          : "Provider disconnected"}
      </div>
    </div>
  </section>

  <div class="context-footer">
    <span>Estimated context</span>
    <strong>8.4k <small>/ 64k tokens</small></strong>
    <div class="context-meter"><span></span></div>
  </div>
</aside>
