<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import { CONVERSATION_GROUPS } from "$lib/presentation";

  type Props = {
    mobileOpen: boolean;
    runtimeVersion: string;
    onclose: () => void;
    onnewchat: () => void;
    onopensettings: () => void;
  };

  let { mobileOpen, runtimeVersion, onclose, onnewchat, onopensettings }: Props = $props();
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
    {#each CONVERSATION_GROUPS as group}
      <section class="conversation-group">
        <h2>{group.label}</h2>
        {#each group.items as conversation}
          <button class:active={conversation.active} class="conversation-item">
            <span>{conversation.title}</span>
            {#if conversation.active}<Icon name="more" size={16} />{/if}
          </button>
        {/each}
      </section>
    {/each}
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
