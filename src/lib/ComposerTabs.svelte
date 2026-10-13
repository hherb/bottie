<script lang="ts">
  /** Accessible composer-mode tabs with controlled selection and standard keyboard navigation. */
  import Icon from "$lib/Icon.svelte";
  type Props = { imageMode: boolean; disabled: boolean; onselect: (image: boolean) => void };
  let { imageMode, disabled, onselect }: Props = $props();
  let chatTab: HTMLButtonElement;
  let imageTab: HTMLButtonElement;

  /** Moves focus and selection between the two tabs without changing mode during active work. */
  function navigate(event: KeyboardEvent) {
    if (disabled || !["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const image = event.key === "End" || (event.key !== "Home" && !imageMode);
    onselect(image);
    (image ? imageTab : chatTab).focus();
  }
</script>

<div class="composer-tabs" role="tablist" aria-label="Composer mode">
  <button
    bind:this={chatTab}
    id="chat-tab"
    type="button"
    role="tab"
    aria-label="Chat"
    aria-selected={!imageMode}
    aria-controls="composer-panel"
    tabindex={imageMode ? -1 : 0}
    {disabled}
    onclick={() => onselect(false)}
    onkeydown={navigate}
  >
    <Icon name="new-chat" size={16} /><span>Chat</span>
  </button>
  <button
    bind:this={imageTab}
    id="image-tab"
    type="button"
    role="tab"
    aria-label="Image"
    aria-selected={imageMode}
    aria-controls="composer-panel"
    tabindex={imageMode ? 0 : -1}
    {disabled}
    onclick={() => onselect(true)}
    onkeydown={navigate}
  >
    <Icon name="image" size={16} /><span>Image</span>
  </button>
</div>
