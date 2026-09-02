import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  msvcCompilationArguments,
  proofProfileLayout,
  runnerBuildArguments,
  safeMsvcDiagnostics,
  safeRunnerStatus,
} from "./windows-python-appcontainer.mjs";

describe("Windows Python AppContainer containment proof", () => {
  it("keeps every executable and runtime byte inside the transient AppContainer profile", () => {
    expect(proofProfileLayout("C:\\Users\\runner\\AppData\\Local\\Packages\\proof\\AC")).toEqual({
      host: "C:\\Users\\runner\\AppData\\Local\\Packages\\proof\\AC\\proof\\bottie-python-appcontainer.exe",
      root: "C:\\Users\\runner\\AppData\\Local\\Packages\\proof\\AC\\proof",
      runner: "C:\\Users\\runner\\AppData\\Local\\Packages\\proof\\AC\\proof\\bottie-python-runner.exe",
      runtime: "C:\\Users\\runner\\AppData\\Local\\Packages\\proof\\AC\\proof\\python",
    });
  });

  it("builds the unchanged locked Rust runner and one warning-clean native proof host", () => {
    expect(runnerBuildArguments("C:\\repo\\python-runner\\Cargo.toml")).toEqual([
      "build",
      "--manifest-path",
      "C:\\repo\\python-runner\\Cargo.toml",
      "--release",
      "--locked",
    ]);
    expect(msvcCompilationArguments("C:\\repo\\windows-python-appcontainer\\Proof.cpp", "C:\\tmp\\proof.exe")).toEqual([
      "/nologo",
      "/std:c++20",
      "/EHsc",
      "/W4",
      "/WX",
      "/DUNICODE",
      "/D_UNICODE",
      "C:\\repo\\windows-python-appcontainer\\Proof.cpp",
      "/Fe:C:\\tmp\\proof.exe",
      "advapi32.lib",
      "ole32.lib",
      "userenv.lib",
    ]);
  });

  it("reduces compiler failures to bounded path-free diagnostic codes and messages", () => {
    expect(
      safeMsvcDiagnostics(
        "C:\\private\\Proof.cpp(42): error C2065: 'missing': undeclared identifier\r\n" +
          "LINK : fatal error LNK1120: 1 unresolved externals\r\n" +
          "C:\\private\\Proof.cpp(43): note: see declaration of 'value'",
      ),
    ).toBe("error C2065: 'missing': undeclared identifier; fatal error LNK1120: 1 unresolved externals");
    expect(safeMsvcDiagnostics("C:\\private\\Proof.cpp was not compiled")).toBe("");
  });

  it("reports only fixed path-free runner statuses", () => {
    expect(safeRunnerStatus("internal_error")).toBe("internal_error");
    expect(safeRunnerStatus("C:\\private\\runtime failed")).toBe("unexpected_result");
  });

  it("launches through an empty-capability AppContainer and a privilege-stripped token", async () => {
    const source = await readFile(new URL("../windows-python-appcontainer/Proof.cpp", import.meta.url), "utf8");
    const token = await readFile(
      new URL("../windows-python-appcontainer/RestrictedToken.hpp", import.meta.url),
      "utf8",
    );
    const storage = await readFile(
      new URL("../windows-python-appcontainer/ProfileStorage.hpp", import.meta.url),
      "utf8",
    );

    expect(source).toContain("CreateAppContainerProfile");
    expect(source).toContain("DeriveAppContainerSidFromAppContainerName");
    expect(source).toContain("ConvertSidToStringSidW");
    expect(source).toContain("GetAppContainerFolderPath(sid_string.c_str()");
    expect(source).toContain("CreateBottieRestrictedToken()");
    expect(token).toContain("CreateRestrictedToken(current, DISABLE_MAX_PRIVILEGE");
    expect(token).toContain("BottieTokenPrivilegesStripped");
    expect(token).toContain("BottieTokenIsLowIntegrity");
    expect(token).toContain("SECURITY_MANDATORY_LOW_RID");
    expect(token).toContain("SE_CHANGE_NOTIFY_NAME");
    expect(source).toContain("security_capabilities.CapabilityCount = 0");
    expect(source).toContain("security_capabilities.Capabilities = nullptr");
    expect(source).toContain("PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES");
    expect(source).toContain("CreateProcessAsUserW");
    expect(source).toContain("PrepareBottieProfileTemp(path, sid.Get())");
    expect(source).toContain("BottieProfileTempPath(profile)");
    expect(storage).toContain("TokenUser");
    expect(storage).toContain('return profile + L"\\\\Temp"');
    expect(storage).toContain("BuildTrusteeWithSidW(&access[0].Trustee, user->User.Sid)");
    expect(storage).toContain("BuildTrusteeWithSidW(&access[1].Trustee, sid)");
    expect(storage).toContain("access[0].Trustee.TrusteeType = TRUSTEE_IS_USER");
    expect(storage).toContain("access[1].Trustee.TrusteeType = TRUSTEE_IS_GROUP");
    expect(storage).toContain("SUB_CONTAINERS_AND_OBJECTS_INHERIT");
    expect(storage).toContain("FILE_DELETE_CHILD");
    expect(storage).toContain('L"S:(ML;OICI;NW;;;LW)"');
    expect(storage).toContain("LABEL_SECURITY_INFORMATION");
    expect(storage).toContain("ProbeBottieTemporaryStorage");
    expect(storage).toContain("CompareStringOrdinal");
    expect(source).toContain("temporaryPathMatchesExpected");
    expect(storage).toContain("result.file_create_error = GetLastError()");
    expect(storage).toContain("WriteFile(file, &kProbeByte");
    expect(storage).toContain("DeleteFileW(file_path.c_str())");
    expect(source).not.toMatch(/internetClient|privateNetworkClientServer|enterpriseAuthentication/);
  });

  it("inherits only private protocol handles and binds the child to a kill-on-close limited job", async () => {
    const source = await readFile(new URL("../windows-python-appcontainer/Proof.cpp", import.meta.url), "utf8");

    expect(source).toContain("PROC_THREAD_ATTRIBUTE_HANDLE_LIST");
    expect(source).toContain("PROC_THREAD_ATTRIBUTE_JOB_LIST");
    expect(source).toContain("JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE");
    expect(source).toContain("JOB_OBJECT_LIMIT_ACTIVE_PROCESS");
    expect(source).toContain("JOB_OBJECT_LIMIT_PROCESS_MEMORY");
    expect(source).toContain("JOB_OBJECT_LIMIT_PROCESS_TIME");
    expect(source).toContain("kProcessMemoryLimitBytes = 768ULL * 1024ULL * 1024ULL");
    expect(source).toContain("kExecutionTimeoutMilliseconds = 150 * 1000");
    expect(source).toContain("120LL * 10LL * 1000LL * 1000LL");
    expect(source).toContain("CreatePipe");
    expect(source).toContain("TerminateJobObject");
    expect(source).toContain("runtime_readable");
    expect(source).toContain("temporary.Writable()");
    expect(source).toContain('Fail("runner_timeout")');
    expect(source).toContain('Fail("runner_exit")');
    expect(source).not.toMatch(/(?:cmd\.exe|powershell|ShellExecute)/i);
  });

  it("runs the credential-free native proof for relevant pull requests", async () => {
    const workflow = await readFile(
      new URL("../.github/workflows/windows-python-appcontainer.yml", import.meta.url),
      "utf8",
    );
    const wrapper = await readFile(new URL("./windows-python-appcontainer.mjs", import.meta.url), "utf8");

    expect(workflow).toContain("pull_request:");
    expect(workflow).toContain("runs-on: windows-2025");
    expect(workflow).toContain("python-runner/runtime-manifest.json");
    expect(workflow).toContain("npm run python:appcontainer:prove");
    expect(workflow).not.toMatch(/environment:|secrets\./);
    expect(wrapper).toContain('runHostCommand(compiled.runner, ["--runtime", runtime]');
    expect(wrapper).toContain('requireOrdinaryResult(baseline, "The uncontained runner control")');
  });
});
