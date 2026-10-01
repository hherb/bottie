<script lang="ts">
  import type { GenerationLimits } from "$lib/inference";

  let {
    limits,
    disabled,
    onchange,
  }: {
    limits: GenerationLimits;
    disabled: boolean;
    onchange: (limits: GenerationLimits) => void;
  } = $props();

  /** Updates one numeric preference while preserving the rest of the Settings draft. */
  function change(field: keyof GenerationLimits, value: string): void {
    onchange({ ...limits, [field]: Number(value) });
  }
</script>

<section class="provider-setting" aria-labelledby="generation-limits-title">
  <div class="provider-setting-heading">
    <span
      ><strong id="generation-limits-title">Generation limits</strong><small>Saved budgets for new answers</small></span
    >
  </div>
  <label for="generation-tool-rounds">Tool rounds</label>
  <input
    id="generation-tool-rounds"
    type="number"
    min="1"
    max="64"
    step="1"
    required
    value={limits.maxToolRounds}
    {disabled}
    oninput={(event) => change("maxToolRounds", event.currentTarget.value)}
  />
  <label for="generation-tool-calls">Total tool calls</label>
  <input
    id="generation-tool-calls"
    type="number"
    min="1"
    max="128"
    step="1"
    required
    value={limits.maxToolCalls}
    {disabled}
    oninput={(event) => change("maxToolCalls", event.currentTarget.value)}
  />
  <p class="credential-status">
    One round can request several tools. Both the round and total call limits apply to each answer.
  </p>
  <label for="generation-output-tokens">Output tokens</label>
  <input
    id="generation-output-tokens"
    type="number"
    min="1"
    max="65536"
    step="1"
    required
    value={limits.maxOutputTokens}
    {disabled}
    oninput={(event) => change("maxOutputTokens", event.currentTarget.value)}
  />
  <p class="credential-status">Maximum output per model request; the model or provider may have a lower limit.</p>
  <p class="credential-status">
    Changes apply after Save and reconnect and survive restart. Tool work retains its five-minute time budget.
  </p>
</section>
