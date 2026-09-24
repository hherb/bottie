// @ts-expect-error Vitest runs this file in Node while the frontend typecheck intentionally excludes Node declarations.
import { readFileSync } from "node:fs";

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

  it("keeps both dark-dialog actions above the WCAG text contrast threshold", () => {
    const approvalStyles = readFileSync(new URL("./styles/python-approval.css", import.meta.url), "utf8");
    const color = (name: string): string => {
      const match = approvalStyles.match(new RegExp(`--${name}:\\s*(#[0-9a-f]{6})`, "i"));
      expect(match, `${name} should be an explicit opaque color`).not.toBeNull();
      return match?.[1] ?? "#000000";
    };

    expect(
      contrast(color("python-approval-button-text"), color("python-approval-button-background")),
    ).toBeGreaterThanOrEqual(4.5);
    expect(
      contrast(color("python-approval-primary-text"), color("python-approval-primary-background")),
    ).toBeGreaterThanOrEqual(4.5);
    expect(approvalStyles).not.toContain("--muted-text");
  });
});

/** Returns the WCAG contrast ratio between two opaque six-digit hexadecimal colors. */
function contrast(left: string, right: string): number {
  const luminance = (value: string): number => {
    const channels = value
      .slice(1)
      .match(/.{2}/g)!
      .map((channel) => Number.parseInt(channel, 16) / 255)
      .map((channel) => (channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4));
    return 0.2126 * channels[0] + 0.7152 * channels[1] + 0.0722 * channels[2];
  };
  const [lighter, darker] = [luminance(left), luminance(right)].sort((a, b) => b - a);
  return (lighter + 0.05) / (darker + 0.05);
}
