<script lang="ts">
  import { untrack } from "svelte";

  import type { WebNetworkPolicy } from "$lib/inference";
  import { cloneWebNetworkPolicy, formatPolicyDomains, parsePolicyDomains } from "$lib/web-policy";

  type Props = {
    policy: WebNetworkPolicy;
    disabled: boolean;
    onchange: (policy: WebNetworkPolicy) => void;
  };

  let { policy, disabled, onchange }: Props = $props();
  let allowedText = $state(untrack(() => formatPolicyDomains(policy.allowedDomains)));
  let blockedText = $state(untrack(() => formatPolicyDomains(policy.blockedDomains)));

  /** Replaces one policy field without sharing mutable domain arrays with active settings. */
  function update(changes: Partial<WebNetworkPolicy>): void {
    onchange({ ...cloneWebNetworkPolicy(policy), ...changes });
  }
</script>

<section class="provider-setting web-network-policy" aria-labelledby="web-network-policy-title">
  <div class="provider-setting-heading">
    <span>
      <strong id="web-network-policy-title">Web destination policy</strong>
      <small>Applied by Rust to search results, fetches, and every redirect</small>
    </span>
  </div>

  <label class="web-policy-toggle" for="web-policy-https-only">
    <input
      id="web-policy-https-only"
      type="checkbox"
      checked={policy.httpsOnly}
      {disabled}
      onchange={(event) => update({ httpsOnly: event.currentTarget.checked })}
    />
    <span>Require HTTPS destinations</span>
  </label>

  <label for="web-policy-allowed-domains">Allowed domains</label>
  <textarea
    id="web-policy-allowed-domains"
    rows="3"
    value={allowedText}
    placeholder="Leave empty to allow any public domain"
    {disabled}
    spellcheck="false"
    oninput={(event) => {
      allowedText = event.currentTarget.value;
      update({ allowedDomains: parsePolicyDomains(allowedText) });
    }}></textarea>
  <p class="credential-status">
    One DNS name per line or comma; parent domains include their subdomains; 32 entries combined.
  </p>

  <label for="web-policy-blocked-domains">Blocked domains</label>
  <textarea
    id="web-policy-blocked-domains"
    rows="3"
    value={blockedText}
    placeholder="Domains Bottie must reject"
    {disabled}
    spellcheck="false"
    oninput={(event) => {
      blockedText = event.currentTarget.value;
      update({ blockedDomains: parsePolicyDomains(blockedText) });
    }}></textarea>
  <p class="web-policy-boundary">
    Private, loopback, special-use, and non-public addresses remain blocked. Blocked domains override allowed domains.
  </p>
</section>
