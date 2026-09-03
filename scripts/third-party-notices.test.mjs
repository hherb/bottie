import { describe, expect, it } from "vitest";

import { buildThirdPartyNotices } from "./third-party-notices.mjs";

describe("third-party notice generation", () => {
  it("sorts package identities and deduplicates byte-identical licence texts", () => {
    const notice = buildThirdPartyNotices({
      packages: [
        { ecosystem: "npm", name: "zeta", version: "2.0.0", licence: "MIT", texts: ["MIT text\n"] },
        { ecosystem: "cargo", name: "alpha", version: "1.0.0", licence: "MIT", texts: ["MIT text\r\n"] },
      ],
      onnxRuntimeLicence: "ORT licence\n",
      onnxRuntimeNotices: "ORT notices\n",
      pythonRuntimeLicence: "CPython licence\n",
      whisperModelLicence: "Whisper model MIT licence\n",
    });

    expect(notice.indexOf("cargo:alpha@1.0.0")).toBeLessThan(notice.indexOf("npm:zeta@2.0.0"));
    expect(notice.match(/MIT text/g)).toHaveLength(1);
    expect(notice).toContain("Used by: cargo:alpha@1.0.0, npm:zeta@2.0.0");
    expect(notice).toContain("ORT licence");
    expect(notice).toContain("ORT notices");
    expect(notice).toContain("CPython licence");
    expect(notice).toContain("Whisper model MIT licence");
    expect(notice.endsWith("\n")).toBe(true);
  });

  it("fails closed when a notice-required package has no distributable text", () => {
    expect(() =>
      buildThirdPartyNotices({
        packages: [{ ecosystem: "cargo", name: "missing", version: "1.0.0", licence: "MIT", texts: [] }],
        onnxRuntimeLicence: "ORT licence\n",
        onnxRuntimeNotices: "ORT notices\n",
        pythonRuntimeLicence: "CPython licence\n",
      }),
    ).toThrow(/missing licence text/);
  });
});
