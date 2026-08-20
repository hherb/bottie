<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { renderAssistantMarkdown } from "$lib/markdown";
  import type { ConversationBranch } from "$lib/storage";
  import type { ModelInfo, ProviderError } from "$lib/inference";
  import type { InferenceStage, Message, ProviderStatus } from "$lib/presentation";

  type Props = {
    messages: Message[];
    providerStatus: ProviderStatus;
    providerError: ProviderError | null;
    selectedModel: ModelInfo | undefined;
    activeStage: number;
    inferenceStages: InferenceStage[];
    isGenerating: boolean;
    branches: ConversationBranch[];
    currentBranchId: string | null;
    onretry: () => void;
    onselectbranch: (branchId: string) => void;
    oneditmessage: (message: Message, text: string) => void;
    onregenerate: (responseId: number) => void;
    onscrollready: (element: HTMLDivElement) => void;
  };

  let {
    messages,
    providerStatus,
    providerError,
    selectedModel,
    activeStage,
    inferenceStages,
    isGenerating,
    branches,
    currentBranchId,
    onretry,
    onselectbranch,
    oneditmessage,
    onregenerate,
    onscrollready,
  }: Props = $props();
  let messageScroll: HTMLDivElement;
  let editingMessageId = $state<number | null>(null);
  let editedText = $state("");

  $effect(() => {
    if (messageScroll) onscrollready(messageScroll);
  });
</script>

<div class="message-scroll" bind:this={messageScroll}>
  {#if providerStatus !== "available"}
    <div class:offline={providerStatus === "offline"} class="provider-banner" role="status">
      <Icon name="shield" size={16} />
      <span>
        <strong>{providerStatus === "checking" ? "Connecting to provider…" : providerError?.message}</strong>
        {#if providerError?.diagnostic}<small>{providerError.diagnostic}</small>{/if}
      </span>
      {#if providerStatus === "offline"}<button onclick={onretry}>Retry</button>{/if}
    </div>
  {/if}
  <div class="conversation-canvas">
    <div class="date-divider"><span>Current conversation</span></div>

    {#if branches.length > 1}
      <label class="branch-picker">
        <span>Conversation branch</span>
        <select
          value={currentBranchId ?? ""}
          disabled={isGenerating}
          onchange={(event) => onselectbranch(event.currentTarget.value)}
        >
          {#each branches as branch, index (branch.id)}
            <option value={branch.id}>{branch.name} · {index + 1} of {branches.length}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#each messages as message (message.id)}
      <article class:assistant={message.role === "assistant"} class:error={message.error} class="message">
        <div class="message-avatar" class:user-avatar={message.role === "user"}>
          {#if message.role === "assistant"}<span class="mini-core"></span>{:else}<span>HH</span>{/if}
        </div>
        <div class="message-content">
          <div class="message-author">
            <strong>{message.role === "assistant" ? "bottie" : "You"}</strong>
            {#if message.role === "assistant"}
              <span>{message.model ?? selectedModel?.displayName ?? "Selected model"}</span>
            {/if}
          </div>

          {#if editingMessageId === message.id}
            <div class="message-editor">
              <textarea bind:value={editedText} rows="3" aria-label="Edit message"></textarea>
              <div>
                <button
                  onclick={() => {
                    editingMessageId = null;
                    editedText = "";
                  }}>Cancel</button
                >
                <button
                  class="primary"
                  disabled={!editedText.trim()}
                  onclick={() => {
                    oneditmessage(message, editedText);
                    editingMessageId = null;
                    editedText = "";
                  }}>Save & regenerate</button
                >
              </div>
            </div>
          {:else}
            <div class:markdown-body={message.role === "assistant"} class="message-text">
              {#if message.role === "assistant"}
                <!-- The renderer emits parser-owned markup with raw HTML and unsafe destinations disabled. -->
                {@html renderAssistantMarkdown(message.content)}
              {:else}
                {#each message.content.split("\n\n") as paragraph}<p>{paragraph}</p>{/each}
              {/if}
              {#if message.content === "" && isGenerating}<span class="typing-caret"></span>{/if}
            </div>
          {/if}

          {#if message.reasoning}
            <details class="reasoning-block">
              <summary>
                <Icon name="brain" size={14} />
                <span>{message.content === "" && isGenerating ? "Thinking…" : "Reasoning"}</span>
                <small>Low effort</small>
              </summary>
              <div class="reasoning-content">
                {#each message.reasoning.split("\n\n") as paragraph}<p>{paragraph}</p>{/each}
              </div>
            </details>
          {/if}

          {#if message.featured}
            <div class="architecture-flow" aria-label="Implementation sequence">
              <div class="flow-step">
                <span class="step-number">01</span>
                <span><strong>Conversation shell</strong><small>Streaming, branching, attachments</small></span>
              </div>
              <div class="flow-line"></div>
              <div class="flow-step">
                <span class="step-number">02</span>
                <span><strong>Rust orchestration</strong><small>Providers, tools, permissions</small></span>
              </div>
              <div class="flow-line"></div>
              <div class="flow-step">
                <span class="step-number">03</span>
                <span><strong>Persistent memory</strong><small>SQLite, FTS5, vector search</small></span>
              </div>
            </div>
            <div class="source-row">
              <button><Icon name="brain" size={14} /> 3 memories</button>
              <button><Icon name="file" size={14} /> 2 attachments</button>
            </div>
          {/if}

          {#if message.role === "user" && message.storageId && editingMessageId !== message.id}
            <div class="message-actions user-message-actions">
              <button
                aria-label="Edit message"
                disabled={isGenerating}
                onclick={() => {
                  editingMessageId = message.id;
                  editedText = message.content;
                }}><Icon name="edit" size={14} /></button
              >
            </div>
          {:else if message.role === "assistant" && message.content !== ""}
            <div class="message-actions">
              <button aria-label="Copy response"><Icon name="copy" size={15} /></button>
              <button aria-label="Good response"><Icon name="thumbs-up" size={15} /></button>
              <button aria-label="Poor response"><Icon name="thumbs-down" size={15} /></button>
              <button aria-label="Regenerate response" disabled={isGenerating} onclick={() => onregenerate(message.id)}
                ><Icon name="refresh" size={15} /></button
              >
              {#if message.meta}<span class="response-meta">{message.meta}</span>{/if}
            </div>
          {/if}
        </div>
      </article>
    {/each}

    {#if activeStage >= 0}
      <div class="activity-card" aria-live="polite">
        <div class="activity-heading">
          <span class="activity-orbit"><span></span></span>
          <strong>
            {activeStage === 0 ? "Starting inference" : `${selectedModel?.providerName ?? "Provider"} is responding`}
          </strong>
        </div>
        <div class="activity-stages">
          {#each inferenceStages as stage, index}
            <div class:current={index === activeStage} class:complete={index < activeStage} class="activity-stage">
              <span class="stage-icon">
                {#if index < activeStage}
                  <Icon name="check" size={13} strokeWidth={2.4} />
                {:else}
                  <Icon name={stage.icon} size={14} />
                {/if}
              </span>
              <span><strong>{stage.label}</strong><small>{stage.detail}</small></span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>
