import { render } from "svelte/server";
import { describe, expect, it } from "vitest";

import PythonApproval from "./PythonApproval.svelte";

describe("PythonApproval", () => {
  it("shows the exact purpose and inert source with one approve and deny action", () => {
    const html = render(PythonApproval, {
      props: {
        approval: {
          requestId: "opaque-native-token",
          phase: "pending",
          source: "<script>alert('no')</script>",
          purpose: "Check a calculation.",
        },
        busy: false,
        error: "",
        ondecide: () => {},
      },
    }).body;

    expect(html).toContain("Python needs your approval");
    expect(html).toContain("Check a calculation.");
    expect(html).toContain("&lt;script>alert('no')&lt;/script>");
    expect(html).not.toContain("<script>");
    expect(html).toContain("Approve once");
    expect(html).toContain("Deny");
    expect(html).toContain("Bottie has not run this code.");
    expect(html).not.toContain("opaque-native-token");
  });

  it("replaces actions with terminal exact-request feedback", () => {
    const html = render(PythonApproval, {
      props: {
        approval: {
          requestId: "opaque-native-token",
          phase: "denied",
          source: "print(4)",
          purpose: "Check a calculation.",
        },
        busy: false,
        error: "",
        ondecide: () => {},
      },
    }).body;

    expect(html).toContain("Denied for this exact request. Bottie did not run the code.");
    expect(html).not.toContain("Approve once");
    expect(html).not.toContain(">Deny<");
  });
});
