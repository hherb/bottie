<script lang="ts">
  type IconName =
    | "arrow-up"
    | "brain"
    | "check"
    | "chevron-down"
    | "copy"
    | "file"
    | "globe"
    | "image"
    | "menu"
    | "more"
    | "new-chat"
    | "paperclip"
    | "panel"
    | "refresh"
    | "search"
    | "settings"
    | "shield"
    | "sparkles"
    | "thumbs-down"
    | "thumbs-up"
    | "x";

  let {
    name,
    size = 18,
    strokeWidth = 1.8,
    className = "",
  }: {
    name: IconName;
    size?: number;
    strokeWidth?: number;
    className?: string;
  } = $props();
</script>

<svg
  class={className}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "arrow-up"}
    <path d="m18 15-6-6-6 6" />
  {:else if name === "brain"}
    <path d="M9.5 4.5A2.5 2.5 0 0 0 7 7v.2A3 3 0 0 0 5 10a3 3 0 0 0 1.2 2.4A3.5 3.5 0 0 0 9.5 17H10V5.5a1 1 0 0 0-.5-1Z" />
    <path d="M14.5 4.5A2.5 2.5 0 0 1 17 7v.2a3 3 0 0 1 2 2.8 3 3 0 0 1-1.2 2.4 3.5 3.5 0 0 1-3.3 4.6H14V5.5a1 1 0 0 1 .5-1Z" />
    <path d="M7 7.2c.6 0 1.2.2 1.6.6M17 7.2c-.6 0-1.2.2-1.6.6M6.2 12.4c.7-.4 1.5-.5 2.2-.2M17.8 12.4c-.7-.4-1.5-.5-2.2-.2" />
  {:else if name === "check"}
    <path d="m5 12 4 4L19 6" />
  {:else if name === "chevron-down"}
    <path d="m7 10 5 5 5-5" />
  {:else if name === "copy"}
    <rect x="9" y="9" width="10" height="10" rx="2" />
    <path d="M15 9V7a2 2 0 0 0-2-2H7a2 2 0 0 0-2 2v6a2 2 0 0 0 2 2h2" />
  {:else if name === "file"}
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v5h5M9 13h6M9 17h4" />
  {:else if name === "globe"}
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
  {:else if name === "image"}
    <rect x="3" y="4" width="18" height="16" rx="3" />
    <circle cx="9" cy="10" r="2" />
    <path d="m21 15-4.5-4.5L7 20" />
  {:else if name === "menu"}
    <path d="M4 7h16M4 12h16M4 17h16" />
  {:else if name === "more"}
    <circle cx="5" cy="12" r="1" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" />
    <circle cx="19" cy="12" r="1" fill="currentColor" stroke="none" />
  {:else if name === "new-chat"}
    <path d="M12 20H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h10" />
    <path d="M16 3h5v5M21 3l-9 9" />
  {:else if name === "paperclip"}
    <path d="m20.5 11.5-8.7 8.7a5 5 0 0 1-7.1-7.1l9.4-9.4a3.5 3.5 0 0 1 5 5l-9.4 9.4a2 2 0 1 1-2.8-2.8l8.6-8.6" />
  {:else if name === "panel"}
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M15 4v16" />
  {:else if name === "refresh"}
    <path d="M20 7v5h-5M4 17v-5h5" />
    <path d="M6.1 9A7 7 0 0 1 18 6l2 1M17.9 15A7 7 0 0 1 6 18l-2-1" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-4-4" />
  {:else if name === "settings"}
    <circle cx="12" cy="12" r="3" />
    <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H3v-4h.2a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1A1.7 1.7 0 0 0 9 4.6 1.7 1.7 0 0 0 10 3V3h4v.2a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z" />
  {:else if name === "shield"}
    <path d="M12 3 5 6v5c0 4.6 2.9 8.2 7 10 4.1-1.8 7-5.4 7-10V6z" />
    <path d="m9 12 2 2 4-4" />
  {:else if name === "sparkles"}
    <path d="m12 3 1.3 3.7L17 8l-3.7 1.3L12 13l-1.3-3.7L7 8l3.7-1.3zM18.5 14l.8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8zM5.5 14l.6 1.4 1.4.6-1.4.6-.6 1.4-.6-1.4-1.4-.6 1.4-.6z" />
  {:else if name === "thumbs-down"}
    <path d="M7 4v11H4a2 2 0 0 1-2-2V7a3 3 0 0 1 3-3zM7 14l4 7 1.5-.8a3 3 0 0 0 1.4-3.6L13.3 15H19a2 2 0 0 0 2-2.4l-1-6A3 3 0 0 0 17 4H7" />
  {:else if name === "thumbs-up"}
    <path d="M7 20V9H4a2 2 0 0 0-2 2v6a3 3 0 0 0 3 3zM7 10l4-7 1.5.8a3 3 0 0 1 1.4 3.6L13.3 9H19a2 2 0 0 1 2 2.4l-1 6A3 3 0 0 1 17 20H7" />
  {:else if name === "x"}
    <path d="m6 6 12 12M18 6 6 18" />
  {/if}
</svg>
