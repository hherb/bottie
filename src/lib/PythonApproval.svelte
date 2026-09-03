<script lang="ts">
  import { tick } from "svelte";

  import { focusFirstModalControl, trapModalFocus } from "$lib/modal-focus";
  import type { PythonApprovalDecision, PythonApprovalStatus } from "$lib/python-approval";

  type Props = {
    approval: PythonApprovalStatus;
    busy: boolean;
    error: string;
    ondecide: (decision: PythonApprovalDecision) => void;
    ondismiss?: () => void;
  };

  let { approval, busy, error, ondecide, ondismiss = () => {} }: Props = $props();
  let dialog = $state<HTMLDivElement>();

  $effect(() => {
    approval.phase;
    void tick().then(() => focusFirstModalControl(dialog));
  });

  /** Traps focus and prevents an undecided request from being dismissed accidentally. */
  function handleWindowKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      if (approval.phase !== "pending") ondismiss();
      return;
    }
    if (event.key === "Tab") trapModalFocus(event, dialog);
  }
</script>

<svelte:window onkeydown={handleWindowKeydown} />

<div class="python-approval-backdrop">
  <div
    bind:this={dialog}
    aria-describedby="python-approval-notice"
    aria-labelledby="python-approval-title"
    aria-modal="true"
    class="python-approval-dialog"
    role="dialog"
  >
    <header>
      <span>Native approval</span>
      <h2 id="python-approval-title">Python needs your approval</h2>
    </header>

    <div class="python-approval-review">
      <h3>Proposed purpose</h3>
      <p>{approval.purpose}</p>
      <h3>Complete proposed source</h3>
      <pre><code>{approval.source}</code></pre>
    </div>

    {#if approval.phase === "pending"}
      <p id="python-approval-notice">
        Bottie has not run this code. Approve once applies only to this exact request; Deny does not run it.
      </p>
      {#if error}<p class="python-approval-error" role="alert">{error}</p>{/if}
      <div class="python-approval-actions">
        <button disabled={busy} onclick={() => ondecide("deny")}>Deny</button>
        <button class="primary" disabled={busy} onclick={() => ondecide("approve")}>
          {busy ? "Recording decision…" : "Approve once"}
        </button>
      </div>
    {:else}
      <p id="python-approval-notice" role="status">
        {approval.phase === "approved"
          ? "Approved once for this exact request. Bottie has still not run the code."
          : "Denied for this exact request. Bottie did not run the code."}
      </p>
      <div class="python-approval-actions">
        <button onclick={ondismiss}>Dismiss</button>
      </div>
    {/if}
  </div>
</div>
