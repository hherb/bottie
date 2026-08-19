<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import ConversationGroup from "$lib/ConversationGroup.svelte";
  import { activeConversationDateGroups, conversationsForLifecycle, type ConversationSummary } from "$lib/storage";

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
    onrenameconversation: (conversationId: string, title: string) => void;
    onarchiveconversation: (conversationId: string, archived: boolean) => void;
    ondeleteconversation: (conversationId: string) => void;
    onrestoreconversation: (conversationId: string) => void;
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
    onrenameconversation,
    onarchiveconversation,
    ondeleteconversation,
    onrestoreconversation,
    onopensettings,
  }: Props = $props();

  let activeGroups = $derived(activeConversationDateGroups(conversations));
  let archivedConversations = $derived(conversationsForLifecycle(conversations, "archived"));
  let deletedConversations = $derived(conversationsForLifecycle(conversations, "deleted"));
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
    {#if storageError}
      <p class="conversation-empty error" role="status">{storageError}</p>
    {/if}
    {#each activeGroups as group (group.label)}
      <ConversationGroup
        label={group.label}
        conversations={group.conversations}
        {activeConversationId}
        disabled={isGenerating}
        onselect={onselectconversation}
        onrename={onrenameconversation}
        onarchive={onarchiveconversation}
        ondelete={ondeleteconversation}
        onrestore={onrestoreconversation}
      />
    {:else}
      <ConversationGroup
        label="Recent"
        conversations={[]}
        {activeConversationId}
        emptyMessage="Your saved conversations will appear here."
        disabled={isGenerating}
        onselect={onselectconversation}
        onrename={onrenameconversation}
        onarchive={onarchiveconversation}
        ondelete={ondeleteconversation}
        onrestore={onrestoreconversation}
      />
    {/each}
    {#if archivedConversations.length > 0}
      <ConversationGroup
        label="Archived"
        conversations={archivedConversations}
        {activeConversationId}
        disabled={isGenerating}
        onselect={onselectconversation}
        onrename={onrenameconversation}
        onarchive={onarchiveconversation}
        ondelete={ondeleteconversation}
        onrestore={onrestoreconversation}
      />
    {/if}
    {#if deletedConversations.length > 0}
      <ConversationGroup
        label="Trash"
        conversations={deletedConversations}
        {activeConversationId}
        disabled={isGenerating}
        onselect={onselectconversation}
        onrename={onrenameconversation}
        onarchive={onarchiveconversation}
        ondelete={ondeleteconversation}
        onrestore={onrestoreconversation}
      />
    {/if}
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
