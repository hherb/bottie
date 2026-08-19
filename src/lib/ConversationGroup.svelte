<script lang="ts">
  import { tick } from "svelte";

  import Icon from "$lib/Icon.svelte";
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
    ondelete: (conversationId: string) => void;
    onrestore: (conversationId: string) => void;
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
    ondelete,
    onrestore,
  }: Props = $props();

  let openMenuId = $state<string | null>(null);
  let renamingId = $state<string | null>(null);
  let renameTitle = $state("");
  let renameInput = $state<HTMLInputElement>();

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

  /** Runs one lifecycle action after closing the disclosure menu. */
  function runAction(action: () => void): void {
    openMenuId = null;
    action();
  }

  /** Closes the action disclosure from any focused action button. */
  function handleActionKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") openMenuId = null;
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
              if (event.key === "Escape") renamingId = null;
            }}
          />
          <button type="submit" aria-label="Save conversation name" disabled={disabled || !renameTitle.trim()}>
            <Icon name="check" size={15} />
          </button>
          <button type="button" aria-label="Cancel rename" onclick={() => (renamingId = null)}>
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
        </button>
        <button
          class="conversation-more"
          aria-label={`Manage ${conversation.title}`}
          aria-expanded={openMenuId === conversation.id}
          {disabled}
          onclick={() => (openMenuId = openMenuId === conversation.id ? null : conversation.id)}
          onkeydown={(event) => {
            if (event.key === "Escape") openMenuId = null;
          }}
        >
          <Icon name="more" size={16} />
        </button>
      {/if}

      {#if openMenuId === conversation.id}
        <div class="conversation-menu" role="group" aria-label={`Actions for ${conversation.title}`}>
          {#if conversation.lifecycle !== "deleted"}
            <button onkeydown={handleActionKeydown} onclick={() => beginRename(conversation)}>Rename</button>
            <button
              onkeydown={handleActionKeydown}
              onclick={() => runAction(() => onarchive(conversation.id, conversation.lifecycle !== "archived"))}
            >
              {conversation.lifecycle === "archived" ? "Unarchive" : "Archive"}
            </button>
            <button
              class="danger"
              onkeydown={handleActionKeydown}
              onclick={() => runAction(() => ondelete(conversation.id))}
            >
              Move to Trash
            </button>
          {:else}
            <button onkeydown={handleActionKeydown} onclick={() => runAction(() => onrestore(conversation.id))}>
              Restore
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {:else}
    {#if emptyMessage}<p class="conversation-empty">{emptyMessage}</p>{/if}
  {/each}
</section>
