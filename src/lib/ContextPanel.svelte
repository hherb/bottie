<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import AttachmentVisual from "$lib/AttachmentVisual.svelte";
  import type { ModelInfo } from "$lib/inference";
  import { attachmentFailure, attachmentStatusLabel } from "$lib/attachment";
  import { attachmentDeliveryLabel } from "$lib/chat";
  import type { MemoryCitation } from "$lib/memory-provenance";
  import type { Attachment, MessageAttachment, ProviderStatus } from "$lib/presentation";
  import type { WebSource } from "$lib/web-provenance";

  type Props = {
    open: boolean;
    attachments: Attachment[];
    conversationAttachments: Attachment[];
    messageAttachments: MessageAttachment[];
    canKeepInConversation: boolean;
    selectedModel: ModelInfo | undefined;
    selectedProviderEndpoint: string;
    providerStatus: ProviderStatus;
    isLocalRoute: boolean;
    webEnabled: boolean;
    webSearchProviderName?: string;
    isAddingAttachments: boolean;
    attachmentFeedback: string | null;
    attachmentFailed: boolean;
    attachmentActionsDisabled: boolean;
    memoryCitations: MemoryCitation[];
    webSources: WebSource[];
    onclose: () => void;
    onadd: () => void;
    onremove: (id: string) => void;
    onkeep: (attachmentId: string) => void;
    onremoveconversation: (attachmentId: string) => void;
    onremovemessage: (messageId: string, attachmentId: string) => void;
    onremovememory: (citationId: string) => void;
    onremovewebsource: (sourceId: string) => void;
  };

  let {
    open,
    attachments,
    conversationAttachments,
    messageAttachments,
    canKeepInConversation,
    selectedModel,
    selectedProviderEndpoint,
    providerStatus,
    isLocalRoute,
    webEnabled,
    webSearchProviderName = "Brave Search",
    isAddingAttachments,
    attachmentFeedback,
    attachmentFailed,
    attachmentActionsDisabled,
    memoryCitations,
    webSources,
    onclose,
    onadd,
    onremove,
    onkeep,
    onremoveconversation,
    onremovemessage,
    onremovememory,
    onremovewebsource,
  }: Props = $props();

  /** Formats trusted native timestamps without exposing any source identity. */
  function citationDate(createdAtMs: number): string {
    return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(createdAtMs);
  }
</script>

<aside
  class:closed={!open}
  class="context-panel"
  aria-label="Conversation context"
  aria-hidden={!open}
  inert={!open ? true : undefined}
>
  <div class="context-header">
    <div>
      <span class="eyebrow">This conversation</span>
      <h2>Context</h2>
    </div>
    <button class="icon-button" aria-label="Close context panel" onclick={onclose}>
      <Icon name="x" size={18} />
    </button>
  </div>

  <div class="context-scroll">
    <section class="context-section">
      <div class="section-heading">
        <h3>
          Attachments <span>{attachments.length + conversationAttachments.length + messageAttachments.length}</span>
        </h3>
        <button disabled={isAddingAttachments} onclick={onadd}>{isAddingAttachments ? "Adding…" : "Add"}</button>
      </div>
      <div class="attachment-list">
        {#each attachments as attachment (attachment.id)}
          {@const failure = attachmentFailure(attachment)}
          <div class:failed={Boolean(failure)} class="attachment-row">
            <AttachmentVisual {attachment} className="attachment-icon" iconSize={18} />
            <span class="attachment-copy">
              <strong>{attachment.name}</strong>
              <small
                >Next message · {attachment.size} ·
                {failure
                  ? "Needs attention"
                  : attachmentStatusLabel(attachment.normalization, attachment.extraction, attachment.indexing)} ·
                {attachmentDeliveryLabel(attachment, selectedModel)}</small
              >
              {#if failure}
                <small class="attachment-error"><span>{failure.title}</span>{failure.detail}</small>
              {/if}
            </span>
            <button
              class="scope-action"
              aria-label={`Keep ${attachment.name} in conversation`}
              disabled={!canKeepInConversation || attachmentActionsDisabled}
              onclick={() => onkeep(attachment.id)}>Keep</button
            >
            <button aria-label={`Remove ${attachment.name}`} onclick={() => onremove(attachment.id)}>
              <Icon name="x" size={15} />
            </button>
          </div>
        {/each}
        {#each conversationAttachments as attachment (attachment.id)}
          {@const failure = attachmentFailure(attachment)}
          <div class:failed={Boolean(failure)} class="attachment-row conversation-association">
            <AttachmentVisual {attachment} className="attachment-icon" iconSize={18} />
            <span class="attachment-copy">
              <strong>{attachment.name}</strong>
              <small>
                Conversation · {attachment.size} ·
                {failure
                  ? "Needs attention"
                  : attachmentStatusLabel(attachment.normalization, attachment.extraction, attachment.indexing)} ·
                {attachmentDeliveryLabel(attachment, selectedModel)}
              </small>
              {#if failure}
                <small class="attachment-error"><span>{failure.title}</span>{failure.detail}</small>
              {/if}
            </span>
            <button
              aria-label={`Remove ${attachment.name} from conversation`}
              disabled={attachmentActionsDisabled}
              onclick={() => onremoveconversation(attachment.id)}
            >
              <Icon name="x" size={15} />
            </button>
          </div>
        {/each}
        {#each messageAttachments as association (`${association.messageId}:${association.attachment.id}`)}
          {@const failure = attachmentFailure(association.attachment)}
          <div class:failed={Boolean(failure)} class="attachment-row retained-association">
            <AttachmentVisual attachment={association.attachment} className="attachment-icon" iconSize={18} />
            <span class="attachment-copy">
              <strong>{association.attachment.name}</strong>
              <small>
                Message · {association.attachment.size} ·
                {failure
                  ? "Needs attention"
                  : attachmentStatusLabel(
                      association.attachment.normalization,
                      association.attachment.extraction,
                      association.attachment.indexing,
                    )} ·
                {attachmentDeliveryLabel(association.attachment, selectedModel)}
              </small>
              {#if failure}
                <small class="attachment-error"><span>{failure.title}</span>{failure.detail}</small>
              {/if}
            </span>
            <button
              aria-label={`Remove ${association.attachment.name} from message`}
              disabled={attachmentActionsDisabled}
              onclick={() => onremovemessage(association.messageId, association.attachment.id)}
            >
              <Icon name="x" size={15} />
            </button>
          </div>
        {/each}
        {#if attachments.length === 0 && conversationAttachments.length === 0 && messageAttachments.length === 0}
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
        <h3>Memories <span>{memoryCitations.length}</span></h3>
        <small class="section-note">Tool-sourced</small>
      </div>
      {#each memoryCitations as citation (citation.id)}
        <div
          class:violet={citation.kind === "conversation"}
          class:cyan={citation.kind === "attachment"}
          class="memory-card"
        >
          <div class="memory-meta">
            <Icon name={citation.kind === "conversation" ? "brain" : "file"} size={14} />
            <span class="memory-kind">{citation.label}</span>
            <button aria-label={`Remove ${citation.title} from context`} onclick={() => onremovememory(citation.id)}>
              <Icon name="x" size={13} />
            </button>
          </div>
          <p>{citation.excerpt}</p>
          <small>{citationDate(citation.createdAtMs)} · {citation.title}</small>
        </div>
      {:else}
        <div class="empty-memories">
          <Icon name="brain" size={18} />
          <span
            ><strong>No recalled context</strong><small>Successful native memory tools add citations here.</small></span
          >
        </div>
      {/each}

      <div class="section-heading web-source-heading">
        <h3>Web sources <span>{webSources.length}</span></h3>
        <small class="section-note">Tool-sourced</small>
      </div>
      {#each webSources as source (source.id)}
        <div class="memory-card web-source-card amber">
          <div class="memory-meta">
            <Icon name="globe" size={14} />
            <span class="memory-kind">{source.label}</span>
            {#if source.cited}<span class="web-citation-status">Cited in response</span>{/if}
            {#if source.untrusted}<span class="web-trust-label">Untrusted content</span>{/if}
            <button aria-label={`Remove ${source.title} from web sources`} onclick={() => onremovewebsource(source.id)}>
              <Icon name="x" size={13} />
            </button>
          </div>
          <a href={source.url} target="_blank" rel="noopener noreferrer">{source.title}</a>
          <p>{source.excerpt}</p>
          {#if source.untrusted}
            <p class="web-trust-note">External page text may contain misleading instructions.</p>
          {/if}
          <small>{source.publishedAt ? `${source.publishedAt} · ` : ""}{source.host}</small>
        </div>
      {:else}
        <div class="empty-memories">
          <Icon name="globe" size={18} />
          <span><strong>No web sources</strong><small>Successful native Web tools add sources here.</small></span>
        </div>
      {/each}
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
            ><strong>This Mac</strong><small
              >{isLocalRoute
                ? webEnabled
                  ? "Model prompt local; search queries leave device"
                  : "Conversation stays local"
                : webEnabled
                  ? "Prompt and search queries leave device"
                  : "Prompt leaves device"}</small
            ></span
          >
          <span>
            <strong>{selectedModel?.providerName ?? "Selected provider"}</strong>
            <small>{selectedProviderEndpoint}</small>
          </span>
        </div>
        <div
          class:cloud={!isLocalRoute || webEnabled}
          class:offline={providerStatus !== "available"}
          class="route-status"
        >
          <span></span>
          {providerStatus === "available"
            ? isLocalRoute
              ? webEnabled
                ? `Loopback model · ${webSearchProviderName} enabled`
                : "Connected over loopback"
              : webEnabled
                ? `Cloud model · ${webSearchProviderName} enabled`
                : "Cloud transmission enabled"
            : "Provider disconnected"}
        </div>
      </div>
    </section>
  </div>

  <div class="context-footer">
    <span>Estimated context</span>
    <strong>8.4k <small>/ 64k tokens</small></strong>
    <div class="context-meter"><span></span></div>
  </div>
</aside>
