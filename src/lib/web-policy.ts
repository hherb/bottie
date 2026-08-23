import type { WebNetworkPolicy } from "./inference";

/** Converts a comma-or-line-delimited draft into ordered domains for native validation. */
export function parsePolicyDomains(value: string): string[] {
  return value
    .split(/[\n,]/u)
    .map((domain) => domain.trim())
    .filter(Boolean);
}

/** Formats normalized native domains for one editable line per entry. */
export function formatPolicyDomains(domains: string[]): string {
  return domains.join("\n");
}

/** Clones nested arrays so a draft cannot mutate the active saved settings object. */
export function cloneWebNetworkPolicy(policy: WebNetworkPolicy): WebNetworkPolicy {
  return {
    httpsOnly: policy.httpsOnly,
    allowedDomains: [...policy.allowedDomains],
    blockedDomains: [...policy.blockedDomains],
  };
}
