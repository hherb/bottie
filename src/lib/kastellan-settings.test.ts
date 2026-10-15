import { describe, expect, it } from "vitest";

import { kastellanDraftChanged, kastellanRouteLabel } from "./kastellan-settings";

const LOCAL = { host: "localhost", cliPath: null, remote: false };
const DGX = { host: "dgx", cliPath: null, remote: true };

describe("Kastellan settings drafts", () => {
  it("treats blank drafts and the defaults as unchanged", () => {
    expect(kastellanDraftChanged(LOCAL, "", "")).toBe(false);
    expect(kastellanDraftChanged(LOCAL, " localhost ", "  ")).toBe(false);
    expect(kastellanDraftChanged(null, "", "")).toBe(false);
  });

  it("detects a changed host or CLI path, including clearing a host", () => {
    expect(kastellanDraftChanged(LOCAL, "dgx", "")).toBe(true);
    expect(kastellanDraftChanged(LOCAL, "", "/opt/kastellan-cli")).toBe(true);
    expect(kastellanDraftChanged(DGX, "dgx", "")).toBe(false);
    expect(kastellanDraftChanged(DGX, "", "")).toBe(true);
  });

  it("describes where requests go", () => {
    expect(kastellanRouteLabel(LOCAL)).toBe("Kastellan on this computer");
    expect(kastellanRouteLabel(DGX)).toBe("dgx over ssh");
  });
});
