<script lang="ts">
  import { formatToolPayload, type StoredToolInvocation } from "$lib/storage";
  import {
    pythonExecutionPresentation,
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
      {@const isPythonTool = tool.toolName === "run_python"}
      {@const pythonReview = pythonToolReview(tool)}
      {@const pythonExecution = pythonExecutionPresentation(tool)}
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
              <h4>{tool.audit.approval?.decision === "approved" ? "Approved purpose" : "Proposed purpose"}</h4>
              <p>{pythonReview.purpose}</p>
              <h4>{tool.audit.approval?.decision === "approved" ? "Approved source" : "Proposed source"}</h4>
              <pre><code>{pythonReview.source}</code></pre>
              {#if tool.audit.approval?.decision === "approved"}
                <p class="python-tool-notice">This exact source was approved once for this response.</p>
              {:else}
                <p class="python-tool-notice">Bottie has not run this code. Approval is required before execution.</p>
              {/if}
            </section>
          {:else if isPythonTool}
            <p class="tool-pending-note">The retained Python proposal could not be presented safely.</p>
          {:else}
            <details class="tool-payload">
              <summary>Arguments</summary>
              <pre>{formatToolPayload(tool.arguments)}</pre>
            </details>
          {/if}
          {#if isPythonTool && pythonExecution}
            <section class="python-execution-result" aria-label="Python execution result">
              {#if pythonExecution.kind === "executed"}
                <div class="python-execution-heading">
                  <h4>Python outcome</h4>
                  <strong>{pythonExecution.statusLabel}</strong>
                </div>
                <div class="python-streams">
                  <div>
                    <h4>Bounded stdout</h4>
                    <pre>{pythonExecution.stdout || "No stdout output."}</pre>
                  </div>
                  <div>
                    <h4>Bounded stderr</h4>
                    <pre>{pythonExecution.stderr || "No stderr output."}</pre>
                  </div>
                </div>
                <dl class="python-execution-meta">
                  <div>
                    <dt>Helper duration</dt>
                    <dd>{pythonExecution.durationLabel}</dd>
                  </div>
                  <div>
                    <dt>Execution provenance</dt>
                    <dd>Bottie’s contained Python runtime</dd>
                  </div>
                </dl>
                <p class="python-execution-provenance">
                  Output is bounded and retained in this selected response’s native audit.
                </p>
              {:else}
                <div class="python-execution-heading">
                  <h4>{pythonExecution.kind === "invalid" ? "Python result unavailable" : "Python outcome"}</h4>
                  {#if pythonExecution.kind !== "invalid"}<strong>{pythonExecution.statusLabel}</strong>{/if}
                </div>
                <p>{pythonExecution.message}</p>
                <p class="python-execution-provenance">
                  Execution provenance is retained in this selected response’s native audit.
                </p>
              {/if}
            </section>
          {:else if tool.result && !isPythonTool}
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
          {:else if !isPythonTool}
            <p class="tool-pending-note">The call has no durable result yet.</p>
          {/if}
        </div>
      </details>
    {/each}
  </div>
</details>
