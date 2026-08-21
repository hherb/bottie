<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import type { Attachment } from "$lib/presentation";

  type Props = {
    attachment: Attachment;
    className: string;
    iconSize: number;
  };

  let { attachment, className, iconSize }: Props = $props();
  let previewFailed = $state(false);
</script>

<span class:image={attachment.kind === "image"} class={className}>
  {#if attachment.previewUrl && !previewFailed}
    <img
      src={attachment.previewUrl}
      alt={`Preview of ${attachment.name}`}
      loading="lazy"
      decoding="async"
      draggable="false"
      onerror={() => (previewFailed = true)}
    />
  {:else}
    <Icon name={attachment.kind} size={iconSize} />
  {/if}
</span>
