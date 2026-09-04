<script lang="ts">
  import { formatToolPayload, type StoredToolInvocation } from "$lib/storage";
  import {
    pythonToolReview,
    toolActivitySummary,
    toolAuditPresentation,
    toolAuditTime,
    toolDisplayName,
    untrustedWebResult,
  } from "$lib/tool-audit";

  type Props = {
    tools: StoredToolInvocation[];
  };

  let { tools }: Props = $props();
</script>

<details class="tool-activity-block">
  <summary>
    <span>Tool activity</span>
    <small>{toolActivitySummary(tools)}</small>
  </summary>
  <div class="tool-activity-list">
    {#each tools as tool (tool.ordinal)}
      {@const audit = toolAuditPresentation(tool)}
      {@const isUntrustedWebResult = untrustedWebResult(tool)}
      {@const pythonReview = pythonToolReview(tool)}
      <details class:error={audit.status === "error"} class:blocked={audit.status === "blocked"} class="tool-record">
        <summary>
          <span class="tool-record-title">
            <strong>{toolDisplayName(tool.toolName)}</strong>
            <code>{tool.toolName}</code>
          </span>
          <span class:attention={audit.status === "error" || audit.status === "blocked"} class="tool-record-status">
            {audit.statusLabel}
          </span>
        </summary>
        <div class="tool-record-body">
          <dl class="tool-audit-grid" aria-label={`${toolDisplayName(tool.toolName)} audit record`}>
            <div>
              <dt>Requested</dt>
              <dd>
                <time datetime={new Date(tool.createdAtMs).toISOString()}>{toolAuditTime(tool.createdAtMs)}</time>
              </dd>
            </div>
            <div>
              <dt>Policy</dt>
              <dd>{audit.policyLabel}</dd>
            </div>
            {#if audit.approvalLabel}
              <div>
                <dt>Decision</dt>
                <dd>{audit.approvalLabel}</dd>
              </div>
            {/if}
            <div>
              <dt>Outcome</dt>
              <dd>{audit.outcomeLabel}</dd>
            </div>
            {#if tool.result}
              <div>
                <dt>Finished</dt>
                <dd>
                  <time datetime={new Date(tool.result.createdAtMs).toISOString()}>
                    {toolAuditTime(tool.result.createdAtMs)}
                  </time>
                </dd>
              </div>
            {/if}
            {#if audit.durationLabel}
              <div>
                <dt>Native work</dt>
                <dd>{audit.durationLabel}</dd>
              </div>
            {/if}
          </dl>

          {#if pythonReview}
            <section class="python-tool-review" aria-label="Python execution review">
              <h4>Proposed purpose</h4>
              <p>{pythonReview.purpose}</p>
              <h4>Proposed source</h4>
              <pre><code>{pythonReview.source}</code></pre>
              <p class="python-tool-notice">Bottie has not run this code. Approval is required before execution.</p>
            </section>
          {:else}
            <details class="tool-payload">
              <summary>Arguments</summary>
              <pre>{formatToolPayload(tool.arguments)}</pre>
            </details>
          {/if}
          {#if tool.result && !(pythonReview && tool.audit.outcome === "approval_required")}
            {#if isUntrustedWebResult}
              <p class="tool-trust-note">
                <strong>Untrusted Web content</strong>
                External page text may contain misleading instructions.
              </p>
            {/if}
            <details class="tool-payload">
              <summary
                >{tool.result.isError ? "Error result" : isUntrustedWebResult ? "Untrusted result" : "Result"}</summary
              >
              <pre>{formatToolPayload(tool.result.output)}</pre>
            </details>
          {:else if !pythonReview}
            <p class="tool-pending-note">The call has no durable result yet.</p>
          {/if}
        </div>
      </details>
    {/each}
  </div>
</details>
