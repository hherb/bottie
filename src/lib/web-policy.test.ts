import { describe, expect, it } from "vitest";

import { cloneWebNetworkPolicy, formatPolicyDomains, parsePolicyDomains } from "./web-policy";

describe("Web policy presentation", () => {
  it("keeps editable domain text path-free and preserves duplicate validation for Rust", () => {
    expect(parsePolicyDomains(" Rust-Lang.org, docs.rs\nRUST-LANG.org ")).toEqual([
      "Rust-Lang.org",
      "docs.rs",
      "RUST-LANG.org",
    ]);
    expect(formatPolicyDomains(["rust-lang.org", "docs.rs"])).toBe("rust-lang.org\ndocs.rs");
  });

  it("clones nested policy arrays before editing a settings draft", () => {
    const source = {
      httpsOnly: true,
      allowedDomains: ["rust-lang.org"],
      blockedDomains: ["ads.rust-lang.org"],
    };
    const cloned = cloneWebNetworkPolicy(source);

    cloned.allowedDomains.push("docs.rs");
    expect(source.allowedDomains).toEqual(["rust-lang.org"]);
  });
});
