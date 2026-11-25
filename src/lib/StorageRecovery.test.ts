import { render } from "svelte/server";
import { describe, expect, it, vi } from "vitest";

import StorageRecovery from "./StorageRecovery.svelte";

describe("StorageRecovery", () => {
  it("announces recovery work and failures without changing recovery policy", () => {
    const restoring = render(StorageRecovery, {
      props: {
        automaticBackupCount: 1,
        latestAutomaticBackupAtMs: 1_777_000_000_000,
        isRestoring: true,
        feedback: "Restoring the selected local backup.",
        failed: false,
        onrestoreautomatic: vi.fn(),
        onrestoremanual: vi.fn(),
      },
    }).body;
    const failed = render(StorageRecovery, {
      props: {
        automaticBackupCount: 0,
        latestAutomaticBackupAtMs: null,
        isRestoring: false,
        feedback: "The selected backup could not be restored.",
        failed: true,
        onrestoreautomatic: vi.fn(),
        onrestoremanual: vi.fn(),
      },
    }).body;

    expect(restoring).toContain('aria-busy="true"');
    expect(restoring).toContain('role="status"');
    expect(failed).toContain('role="alert"');
  });

  it("shows a restore warning in the warning style (#235)", () => {
    const warned = render(StorageRecovery, {
      props: {
        automaticBackupCount: 1,
        latestAutomaticBackupAtMs: 1_777_000_000_000,
        isRestoring: false,
        feedback: "Restored backup · 1 missing file left out",
        failed: false,
        warning: true,
        onrestoreautomatic: vi.fn(),
        onrestoremanual: vi.fn(),
      },
    }).body;

    expect(warned).toMatch(/class="recovery-feedback warning"/);
    expect(warned).toContain('role="status"');
  });
});
