import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  applicationSigningArguments,
  proofBundleLayout,
  runnerSigningArguments,
  serviceBundleMetadata,
  serviceSigningArguments,
  swiftCompilationArguments,
} from "./macos-python-xpc.mjs";

const IDENTITY = "A".repeat(40);

describe("macOS Python XPC containment proof", () => {
  it("places the private service and inherited runner in canonical nested-code locations", () => {
    expect(proofBundleLayout("/tmp/proof.app")).toEqual({
      application: "/tmp/proof.app",
      applicationExecutable: "/tmp/proof.app/Contents/MacOS/bottie-python-xpc-proof",
      applicationInfo: "/tmp/proof.app/Contents/Info.plist",
      runner: "/tmp/proof.app/Contents/XPCServices/com.bottie.python-runner.xpc/Contents/Helpers/bottie-python-runner",
      runtime: "/tmp/proof.app/Contents/XPCServices/com.bottie.python-runner.xpc/Contents/Resources/python",
      service: "/tmp/proof.app/Contents/XPCServices/com.bottie.python-runner.xpc",
      serviceExecutable:
        "/tmp/proof.app/Contents/XPCServices/com.bottie.python-runner.xpc/Contents/MacOS/bottie-python-xpc-service",
      serviceInfo: "/tmp/proof.app/Contents/XPCServices/com.bottie.python-runner.xpc/Contents/Info.plist",
    });
  });

  it("compiles fixed Swift sources without shell interpolation", () => {
    expect(swiftCompilationArguments("service", "/tmp/service", ["A.swift", "Service.swift"])).toEqual([
      "-parse-as-library",
      "-O",
      "-target",
      "arm64-apple-macos14.0",
      "-o",
      "/tmp/service",
      "A.swift",
      "Service.swift",
    ]);
  });

  it("signs inherited runner, restricted service, and outer app independently inside out", () => {
    expect(runnerSigningArguments(IDENTITY, "/tmp/runner", "/repo/Runner.entitlements")).toEqual([
      "--force",
      "--sign",
      IDENTITY,
      "--identifier",
      "com.bottie.python-runner.inherited",
      "--options",
      "runtime",
      "--timestamp=none",
      "--entitlements",
      "/repo/Runner.entitlements",
      "/tmp/runner",
    ]);
    expect(serviceSigningArguments(IDENTITY, "/tmp/service.xpc", "/repo/Service.entitlements")).toEqual([
      "--force",
      "--sign",
      IDENTITY,
      "--options",
      "runtime",
      "--timestamp=none",
      "--entitlements",
      "/repo/Service.entitlements",
      "/tmp/service.xpc",
    ]);
    expect(applicationSigningArguments(IDENTITY, "/tmp/proof.app")).toEqual([
      "--force",
      "--sign",
      IDENTITY,
      "--options",
      "runtime",
      "--timestamp=none",
      "/tmp/proof.app",
    ]);
    for (const arguments_ of [
      runnerSigningArguments(IDENTITY, "/tmp/runner", "/repo/Runner.entitlements"),
      serviceSigningArguments(IDENTITY, "/tmp/service.xpc", "/repo/Service.entitlements"),
      applicationSigningArguments(IDENTITY, "/tmp/proof.app"),
    ]) {
      expect(arguments_).not.toContain("--deep");
    }
  });

  it("declares a private service with one instance tied to its client", () => {
    expect(serviceBundleMetadata()).toEqual({
      CFBundleExecutable: "bottie-python-xpc-service",
      CFBundleIdentifier: "com.bottie.python-runner",
      CFBundleInfoDictionaryVersion: "6.0",
      CFBundleName: "Bottie Python Runner",
      CFBundlePackageType: "XPC!",
      CFBundleShortVersionString: "0.1.0",
      CFBundleVersion: "1",
      LSMinimumSystemVersion: "14.0",
      XPCService: { RunLoopType: "dispatch_main", ServiceType: "Application" },
    });
  });

  it("keeps service and inherited-runner entitlements at their exact least-privilege sets", async () => {
    const service = await readFile(new URL("../macos-python-xpc/Service.entitlements", import.meta.url), "utf8");
    const runner = await readFile(new URL("../macos-python-xpc/Runner.entitlements", import.meta.url), "utf8");

    expect(service).toContain("<key>com.apple.security.app-sandbox</key>");
    expect(service).not.toMatch(/network|files\.|application-groups|temporary-exception|get-task-allow/);
    expect(runner).toContain("<key>com.apple.security.app-sandbox</key>");
    expect(runner).toContain("<key>com.apple.security.inherit</key>");
    expect(runner).not.toMatch(/network|files\.|application-groups|temporary-exception|get-task-allow/);
  });
});
