<script lang="ts">
  import Icon from "$lib/Icon.svelte";
  import type { ProviderError } from "$lib/inference";
  import type { ProviderStatus } from "$lib/presentation";

  type Props = {
    empty: boolean;
    providerStatus: ProviderStatus;
    providerError: ProviderError | null;
    providerName: string | null;
    onretry: () => void;
  };

  let { empty, providerStatus, providerError, providerName, onretry }: Props = $props();

  /** Selects the calm empty-state heading for the current provider readiness. */
  function emptyHeading(status: ProviderStatus): string {
    if (status === "available") return "Ready when you are";
    if (status === "checking") return "Connecting to your provider";
    if (status === "browser") return "Open the Bottie desktop app to begin";
    return "Reconnect to start chatting";
  }

  /** Selects actionable empty-state guidance without changing provider recovery policy. */
  function emptyDetail(status: ProviderStatus, name: string | null): string {
    if (status === "available") {
      return `${name ?? "Your provider"} is connected. Your next message starts a private local conversation.`;
    }
    if (status === "checking") return "Bottie is checking the selected route before messages can be sent.";
    if (status === "browser") return "The browser preview cannot connect to native inference or local storage.";
    return "Your conversation stays available while the selected provider is offline.";
  }
</script>

{#if providerStatus !== "available"}
  <div class:offline={providerStatus === "offline"} class="provider-banner" role="status" aria-live="polite">
    <span class="provider-banner-icon"
      ><Icon name={providerStatus === "checking" ? "sparkles" : "shield"} size={16} /></span
    >
    <span class="provider-banner-copy">
      <strong>
        {providerStatus === "checking"
          ? "Checking provider availability…"
          : (providerError?.message ?? "The selected provider is unavailable.")}
      </strong>
      {#if providerError?.diagnostic}<small>{providerError.diagnostic}</small>{/if}
    </span>
    {#if providerStatus === "offline"}
      <button onclick={onretry}><Icon name="refresh" size={13} /><span>Retry connection</span></button>
    {/if}
  </div>
{/if}

{#if empty}
  <section class:offline={providerStatus === "offline"} class="conversation-empty-state" aria-live="polite">
    <span class="empty-state-mark"><span class="mini-core"></span></span>
    <p>{providerStatus === "available" ? "New private conversation" : "Conversation paused"}</p>
    <h1>{emptyHeading(providerStatus)}</h1>
    <span>{emptyDetail(providerStatus, providerName)}</span>
  </section>
{/if}
