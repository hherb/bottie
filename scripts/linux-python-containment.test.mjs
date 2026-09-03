import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  ProofFailure,
  parseContainmentEvidence,
  runnerBuildArguments,
  safeProcessFailure,
  safeProofFailure,
} from "./linux-python-containment.mjs";

describe("Linux Python containment proof", () => {
  it("builds the unchanged locked Rust runner", () => {
    expect(runnerBuildArguments("/repo/python-runner/Cargo.toml")).toEqual([
      "build",
      "--manifest-path",
      "/repo/python-runner/Cargo.toml",
      "--release",
      "--locked",
    ]);
  });

  it("accepts only the complete path-free containment result", () => {
    expect(
      parseContainmentEvidence(
        JSON.stringify({
          environmentIsolated: true,
          execDenied: true,
          landlockDeniedHostFixture: true,
          networkDenied: true,
          parentDeathSignal: true,
          processCreationDenied: true,
          resourceLimits: true,
          runtimeReadable: true,
          status: "ok",
          workspaceReadable: true,
        }),
      ),
    ).toMatchObject({ status: "ok", execDenied: true, networkDenied: true });
    expect(() => parseContainmentEvidence('{"status":"ok","runtimeReadable":true}')).toThrow(ProofFailure);
    expect(() => parseContainmentEvidence('{"status":"ok","privatePath":"/home/private"}')).toThrow(ProofFailure);
  });

  it("reduces unexpected failures to one path-free diagnostic", () => {
    expect(safeProofFailure(new ProofFailure("The native proof was incomplete."))).toBe(
      "The native proof was incomplete.",
    );
    expect(safeProofFailure(new Error("/home/private/runtime failed"))).toBe("The containment proof failed.");
    expect(safeProcessFailure({ error: { code: "ETIMEDOUT" } })).toBe("timeout");
    expect(safeProcessFailure({ signal: "SIGSYS" })).toBe("signal_sigsys");
    expect(safeProcessFailure({ signal: "SIGSEGV" })).toBe("signal");
    expect(safeProcessFailure({ status: 101, stderr: "BOTTIE_LINUX_STAGE=seccomp\nprivate panic" })).toBe(
      "exit_101_after_seccomp",
    );
    expect(safeProcessFailure({ status: 101, stderr: "/home/private" })).toBe("exit_101");
    expect(safeProcessFailure({ status: 17 })).toBe("exit");
  });

  it("installs the built-in Linux controls before executing generated source", async () => {
    const source = await readFile(new URL("../python-runner/src/linux_containment.rs", import.meta.url), "utf8");
    const runner = await readFile(new URL("../python-runner/src/main.rs", import.meta.url), "utf8");

    expect(source).toContain("landlock_create_ruleset");
    expect(source).toContain("landlock_add_rule");
    expect(source).toContain("landlock_restrict_self");
    expect(source).toContain("/proc/self/task");
    expect(source).toContain("SECCOMP_MODE_FILTER");
    expect(source).toContain("RLIMIT_AS");
    expect(source).toContain("RLIMIT_CPU");
    expect(source).toContain("PR_SET_PDEATHSIG");
    expect(source).toContain("SYS_socket");
    expect(source).toContain("SYS_clone");
    expect(source).toContain("libc::ENOSYS");
    expect(source).toContain("SYS_execve");
    expect(source).toContain("SYS_io_uring_setup");
    expect(runner).toContain('"--linux-contained"');
    expect(runner).toContain('"--linux-containment-probe"');
    expect(runner).not.toMatch(/(?:Command::new|\/bin\/sh|bash -c)/);
  });

  it("runs a credential-free Linux-native proof for relevant pull requests", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/linux-python-containment.yml", import.meta.url),
      "utf8",
    );
    const wrapper = await readFile(new URL("./linux-python-containment.mjs", import.meta.url), "utf8");

    expect(workflow).toContain("pull_request:");
    expect(workflow).toContain("runs-on: ubuntu-24.04");
    expect(workflow).toContain("python-runner/runtime-manifest.json");
    expect(workflow).toContain("npm run python:linux:prove");
    expect(workflow).not.toMatch(/environment:|secrets\./);
    expect(wrapper).toContain('"--linux-contained"');
    expect(wrapper).toContain('"--linux-containment-probe"');
    expect(wrapper).toContain('stdio: ["pipe", "pipe", "pipe"]');
    expect(wrapper).toContain("SIGKILL");
    expect(wrapper).toContain("waitForProcessExit");
  });
});
