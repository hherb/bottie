<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import ConversationGroup from "$lib/ConversationGroup.svelte";
  import {
    activeConversationDateGroups,
    conversationsForLifecycle,
    type ConversationSearchResult,
    type ConversationSummary,
  } from "$lib/storage";

  type Props = {
    mobileOpen: boolean;
    runtimeVersion: string;
    conversations: ConversationSummary[];
    activeConversationId: string | null;
    storageError: string | null;
    searchQuery: string;
    searchResults: ConversationSearchResult[];
    isSearching: boolean;
    isGenerating: boolean;
    onclose: () => void;
    onnewchat: () => void;
    onselectconversation: (conversationId: string) => void;
    onsearch: (query: string) => void;
    onselectsearchresult: (result: ConversationSearchResult) => void;
    onrenameconversation: (conversationId: string, title: string) => void;
    onarchiveconversation: (conversationId: string, archived: boolean) => void;
    onmemoryexclusion: (conversationId: string, excluded: boolean) => void;
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
    searchQuery,
    searchResults,
    isSearching,
    isGenerating,
    onclose,
    onnewchat,
    onselectconversation,
    onsearch,
    onselectsearchresult,
    onrenameconversation,
    onarchiveconversation,
    onmemoryexclusion,
    ondeleteconversation,
    onrestoreconversation,
    onopensettings,
  }: Props = $props();

  let activeGroups = $derived(activeConversationDateGroups(conversations));
  let archivedConversations = $derived(conversationsForLifecycle(conversations, "archived"));
  let deletedConversations = $derived(conversationsForLifecycle(conversations, "deleted"));
  let searchInput = $state<HTMLInputElement>();

  /** Focuses conversation search from the standard desktop shortcut. */
  function handleWindowKeydown(event: KeyboardEvent): void {
    if (event.key.toLowerCase() === "k" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      searchInput?.focus();
      searchInput?.select();
    }
  }

  /** Clears an active query, or releases focus when the field is already empty. */
  function handleSearchKeydown(event: KeyboardEvent): void {
    if (event.key !== "Escape") return;
    if (searchQuery) onsearch("");
    else searchInput?.blur();
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

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

  <label class="conversation-search">
    <Icon name="search" size={17} />
    <input
      aria-label="Search conversations"
      bind:this={searchInput}
      value={searchQuery}
      maxlength="200"
      placeholder="Search conversations"
      oninput={(event) => onsearch(event.currentTarget.value)}
      onkeydown={handleSearchKeydown}
    />
    <kbd>⌘ K</kbd>
  </label>

  <nav class="conversation-list" aria-label="Past conversations">
    {#if storageError}
      <p class="conversation-empty error" role="status">{storageError}</p>
    {/if}
    {#if searchQuery.trim()}
      <section class="conversation-group search-results" aria-live="polite">
        <h2>Search results</h2>
        {#if isSearching}
          <p class="conversation-empty">Searching local conversations…</p>
        {:else if searchResults.length === 0}
          <p class="conversation-empty">No conversations match “{searchQuery.trim()}”.</p>
        {:else}
          {#each searchResults as result (`${result.conversationId}:${result.branchId}`)}
            <button
              class:active={result.conversationId === activeConversationId}
              class="search-result"
              disabled={isGenerating}
              onclick={() => onselectsearchresult(result)}
            >
              <span class="search-result-title">
                <strong>{result.title}</strong>
                {#if result.lifecycle === "archived"}<small>Archived</small>{/if}
              </span>
              <span class="search-result-snippet">{result.snippet}</span>
            </button>
          {/each}
        {/if}
      </section>
    {:else}
      {#each activeGroups as group (group.label)}
        <ConversationGroup
          label={group.label}
          conversations={group.conversations}
          {activeConversationId}
          disabled={isGenerating}
          onselect={onselectconversation}
          onrename={onrenameconversation}
          onarchive={onarchiveconversation}
          {onmemoryexclusion}
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
          {onmemoryexclusion}
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
          {onmemoryexclusion}
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
          {onmemoryexclusion}
          ondelete={ondeleteconversation}
          onrestore={onrestoreconversation}
        />
      {/if}
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
