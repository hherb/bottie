<script lang="ts">
  import { tick } from "svelte";

  import Icon from "$lib/Icon.svelte";
  import { FORGET_CONVERSATION_CONFIRMATION, forgetConversationActionLabel } from "$lib/conversation-forget";
  import { conversationMemoryActionLabel } from "$lib/conversation-memory";
  import type { ConversationSummary } from "$lib/storage";

  type Props = {
    label: string;
    conversations: ConversationSummary[];
    activeConversationId: string | null;
    emptyMessage?: string | null;
    disabled: boolean;
    onselect: (conversationId: string) => void;
    onrename: (conversationId: string, title: string) => void;
    onarchive: (conversationId: string, archived: boolean) => void;
    onmemoryexclusion: (conversationId: string, excluded: boolean) => void;
    ondelete: (conversationId: string) => void;
    onrestore: (conversationId: string) => void;
    onforget: (conversationId: string) => void;
  };

  let {
    label,
    conversations,
    activeConversationId,
    emptyMessage = null,
    disabled,
    onselect,
    onrename,
    onarchive,
    onmemoryexclusion,
    ondelete,
    onrestore,
    onforget,
  }: Props = $props();

  let openMenuId = $state<string | null>(null);
  let renamingId = $state<string | null>(null);
  let renameTitle = $state("");
  let renameInput = $state<HTMLInputElement>();
  let forgettingId = $state<string | null>(null);
  let menuInvoker = $state<HTMLButtonElement | null>(null);

  /** Opens inline title editing for one conversation. */
  async function beginRename(conversation: ConversationSummary): Promise<void> {
    renamingId = conversation.id;
    renameTitle = conversation.title;
    openMenuId = null;
    await tick();
    renameInput?.focus();
    renameInput?.select();
  }

  /** Submits a non-empty title and closes the inline editor. */
  function submitRename(): void {
    const title = renameTitle.trim();
    if (!renamingId || !title) return;
    onrename(renamingId, title);
    renamingId = null;
    renameTitle = "";
  }

  /** Cancels inline title editing and returns focus to the conversation action control. */
  async function cancelRename(conversationId: string): Promise<void> {
    renamingId = null;
    renameTitle = "";
    await tick();
    document.querySelector<HTMLButtonElement>(`#conversation-manage-${conversationId}`)?.focus();
  }

  /** Runs one lifecycle action after closing the disclosure menu. */
  function runAction(action: () => void): void {
    openMenuId = null;
    forgettingId = null;
    action();
  }

  /** Opens or closes one action menu while clearing any stale forget confirmation. */
  async function toggleMenu(conversationId: string, invoker: HTMLButtonElement): Promise<void> {
    const shouldOpen = openMenuId !== conversationId;
    openMenuId = shouldOpen ? conversationId : null;
    forgettingId = null;
    menuInvoker = shouldOpen ? invoker : null;
    if (!shouldOpen) return;
    await tick();
    document.querySelector<HTMLButtonElement>(`#conversation-actions-${conversationId} button`)?.focus();
  }

  /** Closes the action disclosure and restores the control that opened it. */
  async function handleActionKeydown(event: KeyboardEvent): Promise<void> {
    if (event.key === "Escape") {
      event.preventDefault();
      openMenuId = null;
      forgettingId = null;
      await tick();
      menuInvoker?.focus();
      menuInvoker = null;
    }
  }
</script>

<section class="conversation-group">
  <h2>{label}</h2>
  {#each conversations as conversation (conversation.id)}
    <div class="conversation-row" class:menu-open={openMenuId === conversation.id}>
      {#if renamingId === conversation.id}
        <form
          class="conversation-rename"
          onsubmit={(event) => {
            event.preventDefault();
            submitRename();
          }}
        >
          <input
            aria-label={`Rename ${conversation.title}`}
            bind:this={renameInput}
            bind:value={renameTitle}
            {disabled}
            maxlength="80"
            onkeydown={(event) => {
              if (event.key === "Escape") void cancelRename(conversation.id);
            }}
          />
          <button type="submit" aria-label="Save conversation name" disabled={disabled || !renameTitle.trim()}>
            <Icon name="check" size={15} />
          </button>
          <button type="button" aria-label="Cancel rename" onclick={() => void cancelRename(conversation.id)}>
            <Icon name="x" size={14} />
          </button>
        </form>
      {:else}
        <button
          class:active={conversation.id === activeConversationId}
          class="conversation-item"
          disabled={disabled || conversation.lifecycle === "deleted"}
          onclick={() => onselect(conversation.id)}
        >
          <span>{conversation.title}</span>
          {#if conversation.memoryExcluded}<small class="memory-excluded">Memory off</small>{/if}
        </button>
        <button
          id={`conversation-manage-${conversation.id}`}
          class="conversation-more"
          aria-label={`Manage ${conversation.title}`}
          aria-expanded={openMenuId === conversation.id}
          aria-controls={`conversation-actions-${conversation.id}`}
          {disabled}
          onclick={(event) => void toggleMenu(conversation.id, event.currentTarget)}
          onkeydown={(event) => void handleActionKeydown(event)}
        >
          <Icon name="more" size={16} />
        </button>
      {/if}

      {#if openMenuId === conversation.id}
        <div
          id={`conversation-actions-${conversation.id}`}
          class="conversation-menu"
          class:forget-open={forgettingId === conversation.id}
          role="group"
          aria-label={`Actions for ${conversation.title}`}
        >
          {#if conversation.lifecycle !== "deleted"}
            <button onkeydown={(event) => void handleActionKeydown(event)} onclick={() => beginRename(conversation)}
              >Rename</button
            >
            <button
              onkeydown={(event) => void handleActionKeydown(event)}
              onclick={() => runAction(() => onarchive(conversation.id, conversation.lifecycle !== "archived"))}
            >
              {conversation.lifecycle === "archived" ? "Unarchive" : "Archive"}
            </button>
            <button
              onkeydown={(event) => void handleActionKeydown(event)}
              onclick={() => runAction(() => onmemoryexclusion(conversation.id, !conversation.memoryExcluded))}
            >
              {conversationMemoryActionLabel(conversation)}
            </button>
            <button
              class="danger"
              onkeydown={(event) => void handleActionKeydown(event)}
              onclick={() => runAction(() => ondelete(conversation.id))}
            >
              Move to Trash
            </button>
          {:else}
            {#if forgettingId === conversation.id}
              <div class="forget-confirmation" role="alert">
                <p>{FORGET_CONVERSATION_CONFIRMATION}</p>
                <div class="forget-confirmation-actions">
                  <button onkeydown={(event) => void handleActionKeydown(event)} onclick={() => (forgettingId = null)}
                    >Cancel</button
                  >
                  <button
                    class="danger"
                    onkeydown={(event) => void handleActionKeydown(event)}
                    onclick={() => runAction(() => onforget(conversation.id))}
                  >
                    Forget forever
                  </button>
                </div>
              </div>
            {:else}
              <button
                onkeydown={(event) => void handleActionKeydown(event)}
                onclick={() => runAction(() => onrestore(conversation.id))}
              >
                Restore
              </button>
              <button
                class="danger"
                onkeydown={(event) => void handleActionKeydown(event)}
                onclick={() => (forgettingId = conversation.id)}
              >
                {forgetConversationActionLabel()}
              </button>
            {/if}
          {/if}
        </div>
      {/if}
    </div>
  {:else}
    {#if emptyMessage}<p class="conversation-empty">{emptyMessage}</p>{/if}
  {/each}
</section>
