<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import type { ConversationSummary } from "$lib/storage";

  type Props = {
    mobileOpen: boolean;
    runtimeVersion: string;
    conversations: ConversationSummary[];
    activeConversationId: string | null;
    storageError: string | null;
    isGenerating: boolean;
    onclose: () => void;
    onnewchat: () => void;
    onselectconversation: (conversationId: string) => void;
    onopensettings: () => void;
  };

  let {
    mobileOpen,
    runtimeVersion,
    conversations,
    activeConversationId,
    storageError,
    isGenerating,
    onclose,
    onnewchat,
    onselectconversation,
    onopensettings,
  }: Props = $props();
</script>

{#if mobileOpen}
  <button class="mobile-scrim" aria-label="Close conversations" onclick={onclose}></button>
{/if}

<aside class:mobile-open={mobileOpen} class="sidebar" aria-label="Conversation navigation">
  <div class="brand-row">
    <div class="brand-mark" aria-hidden="true"><span class="brand-core"></span></div>
    <span class="brand-name">bottie</span>
    <span class="alpha-label">alpha</span>
  </div>

  <button class="new-chat" onclick={onnewchat}>
    <Icon name="new-chat" size={17} />
    <span>New conversation</span>
    <kbd>⌘ N</kbd>
  </button>

  <button class="search-memory">
    <Icon name="search" size={17} />
    <span>Search memory</span>
    <kbd>⌘ K</kbd>
  </button>

  <nav class="conversation-list" aria-label="Past conversations">
    <section class="conversation-group">
      <h2>Recent</h2>
      {#each conversations as conversation (conversation.id)}
        <button
          class:active={conversation.id === activeConversationId}
          class="conversation-item"
          disabled={isGenerating}
          onclick={() => onselectconversation(conversation.id)}
        >
          <span>{conversation.title}</span>
          {#if conversation.id === activeConversationId}<Icon name="more" size={16} />{/if}
        </button>
      {:else}
        <p class:error={storageError !== null} class="conversation-empty">
          {storageError ?? "Your saved conversations will appear here."}
        </p>
      {/each}
    </section>
  </nav>

  <div class="sidebar-footer">
    <button class="settings-button" onclick={onopensettings}>
      <Icon name="settings" size={18} />
      <span>Settings</span>
    </button>
    <button class="profile-button" aria-label="Open profile settings">
      <span class="avatar">HH</span>
      <span class="profile-copy">
        <strong>Local profile</strong>
        <small>{runtimeVersion === "preview" ? "Browser preview" : `bottie ${runtimeVersion}`}</small>
      </span>
      <Icon name="more" size={17} />
    </button>
  </div>
</aside>
