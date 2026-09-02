#define WIN32_LEAN_AND_MEAN
#define _WIN32_WINNT 0x0A00
#include <windows.h>
#include <sddl.h>
#include <userenv.h>
#include "ProfileStorage.hpp"
#include "RestrictedToken.hpp"
#include <algorithm>
#include <array>
#include <cstddef>
#include <future>
#include <iostream>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>
namespace {
constexpr DWORD kMaximumRequestBytes = 256 * 1024;
constexpr DWORD kMaximumResponseBytes = 128 * 1024;
constexpr DWORD kExecutionTimeoutMilliseconds = 150 * 1000;
constexpr DWORD kCancellationDelayMilliseconds = 250;
constexpr SIZE_T kProcessMemoryLimitBytes = 768ULL * 1024ULL * 1024ULL;
constexpr LONGLONG kProcessCpuLimit100Nanoseconds =
    120LL * 10LL * 1000LL * 1000LL;
class Handle final {
public:
  Handle() = default;
  explicit Handle(HANDLE value) : value_(value) {}
  ~Handle() { Reset(); }
  Handle(const Handle &) = delete;
  Handle &operator=(const Handle &) = delete;
  Handle(Handle &&other) noexcept
      : value_(std::exchange(other.value_, nullptr)) {}
  Handle &operator=(Handle &&other) noexcept {
    if (this != &other)
      Reset(std::exchange(other.value_, nullptr));
    return *this;
  }
  [[nodiscard]] HANDLE Get() const { return value_; }
  [[nodiscard]] HANDLE *Out() {
    Reset();
    return &value_;
  }
  void Reset(HANDLE value = nullptr) {
    if (value_ != nullptr && value_ != INVALID_HANDLE_VALUE)
      CloseHandle(value_);
    value_ = value;
  }
private:
  HANDLE value_ = nullptr;
};
class Sid final {
public:
  ~Sid() {
    if (value_ != nullptr)
      FreeSid(value_);
  }
  Sid(const Sid &) = delete;
  Sid &operator=(const Sid &) = delete;
  Sid() = default;
  Sid(Sid &&other) noexcept : value_(std::exchange(other.value_, nullptr)) {}
  Sid &operator=(Sid &&other) noexcept {
    if (this != &other) {
      if (value_ != nullptr)
        FreeSid(value_);
      value_ = std::exchange(other.value_, nullptr);
    }
    return *this;
  }
  [[nodiscard]] PSID Get() const { return value_; }
  [[nodiscard]] PSID *Out() { return &value_; }
private:
  PSID value_ = nullptr;
};
[[noreturn]] void Fail(std::string_view stage = "native") {
  throw std::runtime_error(std::string(stage));
}
void Require(BOOL value) {
  if (!value)
    Fail();
}
void RequireHr(HRESULT value) {
  if (FAILED(value))
    Fail();
}
std::string Utf8(std::wstring_view value) {
  if (value.empty())
    return {};
  const int length = WideCharToMultiByte(CP_UTF8, 0, value.data(),
                                         static_cast<int>(value.size()),
                                         nullptr, 0, nullptr, nullptr);
  if (length <= 0)
    Fail();
  std::string encoded(static_cast<std::size_t>(length), '\0');
  Require(WideCharToMultiByte(CP_UTF8, 0, value.data(),
                              static_cast<int>(value.size()), encoded.data(),
                              length, nullptr, nullptr) > 0);
  return encoded;
}
std::string JsonEscape(std::wstring_view value) {
  std::string escaped;
  for (const char character : Utf8(value)) {
    if (character == '\\' || character == '"')
      escaped.push_back('\\');
    escaped.push_back(character);
  }
  return escaped;
}
bool ValidMoniker(std::wstring_view value) {
  return !value.empty() && value.size() <= 64 &&
         std::all_of(value.begin(), value.end(), [](wchar_t character) {
           return (character >= L'a' && character <= L'z') ||
                  (character >= L'0' && character <= L'9') || character == L'.';
         });
}
std::wstring ProfilePath(PSID sid) {
  PWSTR raw_sid = nullptr;
  Require(ConvertSidToStringSidW(sid, &raw_sid));
  const std::wstring sid_string(raw_sid);
  LocalFree(raw_sid);
  PWSTR raw_path = nullptr;
  RequireHr(GetAppContainerFolderPath(sid_string.c_str(), &raw_path));
  const std::wstring path(raw_path);
  CoTaskMemFree(raw_path);
  return path;
}
Sid DeriveSid(std::wstring_view moniker) {
  Sid sid;
  RequireHr(DeriveAppContainerSidFromAppContainerName(
      std::wstring(moniker).c_str(), sid.Out()));
  return sid;
}
void Prepare(std::wstring_view moniker) {
  if (!ValidMoniker(moniker))
    Fail();
  Sid sid;
  const HRESULT created = CreateAppContainerProfile(
      std::wstring(moniker).c_str(), L"Bottie Python proof",
      L"Transient development containment proof", nullptr, 0, sid.Out());
  if (created == HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS))
    sid = DeriveSid(moniker);
  else
    RequireHr(created);
  const std::wstring path = ProfilePath(sid.Get());
  Require(PrepareBottieProfileTemp(path, sid.Get()));
  std::cout << "{\"profilePath\":\"" << JsonEscape(path)
            << "\",\"status\":\"prepared\"}\n";
}
void Cleanup(std::wstring_view moniker) {
  if (!ValidMoniker(moniker))
    Fail();
  const HRESULT deleted =
      DeleteAppContainerProfile(std::wstring(moniker).c_str());
  if (FAILED(deleted) && deleted != HRESULT_FROM_WIN32(ERROR_NOT_FOUND) &&
      deleted != HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND))
    Fail();
  std::cout << "{\"status\":\"cleaned\"}\n";
}
std::wstring Quote(std::wstring_view argument) {
  std::wstring quoted = L"\"";
  std::size_t slashes = 0;
  for (const wchar_t character : argument) {
    if (character == L'\\') {
      ++slashes;
      continue;
    }
    if (character == L'"')
      quoted.append(slashes * 2 + 1, L'\\');
    else
      quoted.append(slashes, L'\\');
    slashes = 0;
    quoted.push_back(character);
  }
  quoted.append(slashes * 2, L'\\');
  quoted.push_back(L'"');
  return quoted;
}
std::wstring CommandLine(const std::wstring &executable,
                         const std::vector<std::wstring> &arguments) {
  std::wstring command = Quote(executable);
  for (const auto &argument : arguments) {
    command.push_back(L' ');
    command.append(Quote(argument));
  }
  return command;
}
Handle LimitedJob() {
  Handle job(CreateJobObjectW(nullptr, nullptr));
  if (job.Get() == nullptr)
    Fail();
  JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits{};
  limits.BasicLimitInformation.LimitFlags =
      JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_ACTIVE_PROCESS |
      JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_PROCESS_TIME;
  limits.BasicLimitInformation.ActiveProcessLimit = 1;
  limits.BasicLimitInformation.PerProcessUserTimeLimit.QuadPart =
      kProcessCpuLimit100Nanoseconds;
  limits.ProcessMemoryLimit = kProcessMemoryLimitBytes;
  Require(SetInformationJobObject(job.Get(), JobObjectExtendedLimitInformation,
                                  &limits, sizeof(limits)));
  return job;
}
struct PipePair {
  Handle child;
  Handle parent;
};
PipePair InputPipe() {
  SECURITY_ATTRIBUTES attributes{sizeof(attributes), nullptr, TRUE};
  PipePair pipe;
  Require(CreatePipe(pipe.child.Out(), pipe.parent.Out(), &attributes, 0));
  Require(SetHandleInformation(pipe.parent.Get(), HANDLE_FLAG_INHERIT, 0));
  return pipe;
}
PipePair OutputPipe() {
  SECURITY_ATTRIBUTES attributes{sizeof(attributes), nullptr, TRUE};
  PipePair pipe;
  Require(CreatePipe(pipe.parent.Out(), pipe.child.Out(), &attributes, 0));
  Require(SetHandleInformation(pipe.parent.Get(), HANDLE_FLAG_INHERIT, 0));
  return pipe;
}
std::vector<wchar_t> MinimalEnvironment(const std::wstring &profile) {
  const std::wstring temporary = BottieProfileTempPath(profile);
  std::wstring block = L"LOCALAPPDATA=" + profile;
  block.push_back(L'\0');
  block.append(L"TEMP=").append(temporary);
  block.push_back(L'\0');
  block.append(L"TMP=").append(temporary);
  block.push_back(L'\0');
  block.push_back(L'\0');
  return {block.begin(), block.end()};
}
struct Execution {
  Handle job;
  Handle process;
  Handle standard_input;
  Handle standard_output;
  Handle standard_error;
  DWORD process_identifier = 0;
};
Execution Launch(std::wstring_view moniker, const std::wstring &executable,
                 const std::vector<std::wstring> &arguments) {
  Sid sid = DeriveSid(moniker);
  Handle token(CreateBottieRestrictedToken());
  if (token.Get() == nullptr)
    Fail();
  Handle job = LimitedJob();
  PipePair input = InputPipe();
  PipePair output = OutputPipe();
  PipePair error = OutputPipe();
  std::array<HANDLE, 3> inherited{input.child.Get(), output.child.Get(),
                                  error.child.Get()};
  std::array<HANDLE, 1> jobs{job.Get()};
  SECURITY_CAPABILITIES security_capabilities{};
  security_capabilities.AppContainerSid = sid.Get();
  security_capabilities.CapabilityCount = 0;
  security_capabilities.Capabilities = nullptr;
  SIZE_T attribute_bytes = 0;
  InitializeProcThreadAttributeList(nullptr, 3, 0, &attribute_bytes);
  if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
    Fail();
  std::vector<std::byte> attributes(attribute_bytes);
  auto *attribute_list =
      reinterpret_cast<LPPROC_THREAD_ATTRIBUTE_LIST>(attributes.data());
  Require(InitializeProcThreadAttributeList(attribute_list, 3, 0,
                                            &attribute_bytes));
  Require(UpdateProcThreadAttribute(
      attribute_list, 0, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
      &security_capabilities, sizeof(security_capabilities), nullptr, nullptr));
  Require(UpdateProcThreadAttribute(
      attribute_list, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, inherited.data(),
      sizeof(inherited), nullptr, nullptr));
  Require(UpdateProcThreadAttribute(attribute_list, 0,
                                    PROC_THREAD_ATTRIBUTE_JOB_LIST, jobs.data(),
                                    sizeof(jobs), nullptr, nullptr));
  STARTUPINFOEXW startup{};
  startup.StartupInfo.cb = sizeof(startup);
  startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
  startup.StartupInfo.hStdInput = input.child.Get();
  startup.StartupInfo.hStdOutput = output.child.Get();
  startup.StartupInfo.hStdError = error.child.Get();
  startup.lpAttributeList = attribute_list;
  PROCESS_INFORMATION process{};
  std::wstring command = CommandLine(executable, arguments);
  std::vector<wchar_t> environment = MinimalEnvironment(ProfilePath(sid.Get()));
  const DWORD flags = EXTENDED_STARTUPINFO_PRESENT |
                      CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED |
                      CREATE_NO_WINDOW;
  const BOOL created = CreateProcessAsUserW(
      token.Get(), executable.c_str(), command.data(), nullptr, nullptr, TRUE,
      flags, environment.data(), nullptr, &startup.StartupInfo, &process);
  DeleteProcThreadAttributeList(attribute_list);
  Require(created);
  Handle process_handle(process.hProcess);
  Handle thread(process.hThread);
  Require(ResumeThread(thread.Get()) != static_cast<DWORD>(-1));
  return {std::move(job),          std::move(process_handle),
          std::move(input.parent), std::move(output.parent),
          std::move(error.parent), process.dwProcessId};
}
std::string ReadPipe(HANDLE pipe) {
  std::string captured;
  std::array<char, 4096> buffer{};
  for (;;) {
    DWORD read = 0;
    if (!ReadFile(pipe, buffer.data(), static_cast<DWORD>(buffer.size()), &read,
                  nullptr)) {
      if (GetLastError() == ERROR_BROKEN_PIPE)
        break;
      Fail();
    }
    if (read == 0)
      break;
    if (captured.size() + read > kMaximumResponseBytes)
      Fail();
    captured.append(buffer.data(), read);
  }
  return captured;
}
std::string ReadRequest() {
  std::string request;
  std::array<char, 4096> buffer{};
  for (;;) {
    std::cin.read(buffer.data(), static_cast<std::streamsize>(buffer.size()));
    const std::streamsize count = std::cin.gcount();
    if (count > 0)
      request.append(buffer.data(), static_cast<std::size_t>(count));
    if (request.size() > kMaximumRequestBytes)
      Fail();
    if (count < static_cast<std::streamsize>(buffer.size()))
      break;
  }
  if (request.empty())
    Fail();
  return request;
}
void WriteRequest(Execution &execution, const std::string &request) {
  std::size_t offset = 0;
  while (offset < request.size()) {
    DWORD written = 0;
    const DWORD remaining = static_cast<DWORD>(
        std::min<std::size_t>(request.size() - offset, MAXDWORD));
    Require(WriteFile(execution.standard_input.Get(), request.data() + offset,
                      remaining, &written, nullptr));
    if (written == 0)
      Fail();
    offset += written;
  }
  execution.standard_input.Reset();
}
std::string Complete(Execution &execution) {
  auto output =
      std::async(std::launch::async, ReadPipe, execution.standard_output.Get());
  auto error =
      std::async(std::launch::async, ReadPipe, execution.standard_error.Get());
  if (WaitForSingleObject(execution.process.Get(),
                          kExecutionTimeoutMilliseconds) != WAIT_OBJECT_0) {
    TerminateJobObject(execution.job.Get(), ERROR_TIMEOUT);
    Fail("runner_timeout");
  }
  DWORD exit_code = 0;
  Require(GetExitCodeProcess(execution.process.Get(), &exit_code));
  const std::string result = output.get();
  static_cast<void>(error.get());
  if (exit_code != 0)
    Fail("runner_exit");
  if (result.empty())
    Fail("runner_empty_result");
  return result;
}
Execution LaunchRunner(std::wstring_view moniker, const std::wstring &runner,
                       const std::wstring &runtime,
                       const std::string &request) {
  Execution execution = Launch(moniker, runner, {L"--runtime", runtime});
  WriteRequest(execution, request);
  return execution;
}
void Execute(std::wstring_view moniker, const std::wstring &runner,
             const std::wstring &runtime) {
  Execution execution = LaunchRunner(moniker, runner, runtime, ReadRequest());
  std::cout << Complete(execution);
}
void Cancel(std::wstring_view moniker, const std::wstring &runner,
            const std::wstring &runtime) {
  Execution execution = LaunchRunner(moniker, runner, runtime, ReadRequest());
  Sleep(kCancellationDelayMilliseconds);
  Require(TerminateJobObject(execution.job.Get(), ERROR_CANCELLED));
  Require(WaitForSingleObject(execution.process.Get(),
                              kExecutionTimeoutMilliseconds) == WAIT_OBJECT_0);
  std::cout << "{\"status\":\"cancelled\"}\n";
}
void StartAndExit(std::wstring_view moniker, const std::wstring &runner,
                  const std::wstring &runtime) {
  Execution execution = LaunchRunner(moniker, runner, runtime, ReadRequest());
  std::cout << "{\"pid\":" << execution.process_identifier
            << ",\"status\":\"started\"}\n"
            << std::flush;
  ExitProcess(0);
}
void ContainedProbe(const std::wstring &fixture, const std::wstring &runtime,
                    const std::wstring &temporary_path) {
  Handle token;
  Require(OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, token.Out()));
  BOOL is_app_container = FALSE;
  DWORD returned = 0;
  Require(GetTokenInformation(token.Get(), TokenIsAppContainer,
                              &is_app_container, sizeof(is_app_container),
                              &returned));
  DWORD capability_bytes = 0;
  GetTokenInformation(token.Get(), TokenCapabilities, nullptr, 0,
                      &capability_bytes);
  if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
    Fail();
  std::vector<std::byte> capabilities(capability_bytes);
  Require(GetTokenInformation(token.Get(), TokenCapabilities,
                              capabilities.data(), capability_bytes,
                              &returned));
  const auto *groups =
      reinterpret_cast<const TOKEN_GROUPS *>(capabilities.data());
  Handle file(CreateFileW(fixture.c_str(), GENERIC_READ, FILE_SHARE_READ,
                          nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL,
                          nullptr));
  const bool denied = file.Get() == INVALID_HANDLE_VALUE;
  const std::wstring runtime_file = runtime + L"\\python.wasm";
  Handle runtime_handle(CreateFileW(runtime_file.c_str(), GENERIC_READ,
                                    FILE_SHARE_READ, nullptr, OPEN_EXISTING,
                                    FILE_ATTRIBUTE_NORMAL, nullptr));
  const bool runtime_readable = runtime_handle.Get() != INVALID_HANDLE_VALUE;
  const BottieTemporaryStorageProbe temporary =
      ProbeBottieTemporaryStorage(temporary_path);
  const bool privileges_stripped = BottieTokenPrivilegesStripped(token.Get());
  const bool low_integrity = BottieTokenIsLowIntegrity(token.Get());
  const bool ok = is_app_container == TRUE && privileges_stripped &&
                  low_integrity && groups->GroupCount == 0 && denied &&
                  runtime_readable && temporary.path_matches_expected &&
                  temporary.Writable();
  std::cout << "{\"appContainer\":" << (is_app_container ? "true" : "false")
            << ",\"capabilityCount\":" << groups->GroupCount
            << ",\"hostFixtureDenied\":" << (denied ? "true" : "false")
            << ",\"lowIntegrity\":" << (low_integrity ? "true" : "false")
            << ",\"privilegesStripped\":"
            << (privileges_stripped ? "true" : "false")
            << ",\"runtimeReadable\":" << (runtime_readable ? "true" : "false")
            << ",\"temporaryCreateError\":" << temporary.file_create_error
            << ",\"temporaryFileCreated\":" << (temporary.file_created ? "true" : "false")
            << ",\"temporaryFileDeleted\":"
            << (temporary.file_deleted ? "true" : "false")
            << ",\"temporaryFileWritten\":"
            << (temporary.file_written ? "true" : "false")
            << ",\"temporaryPathAvailable\":"
            << (temporary.path_available ? "true" : "false")
            << ",\"temporaryPathMatchesExpected\":"
            << (temporary.path_matches_expected ? "true" : "false")
            << ",\"temporaryWritable\":"
            << (temporary.Writable() ? "true" : "false")
            << ",\"status\":\"" << (ok ? "ok" : "failed") << "\"}\n";
}
void Probe(std::wstring_view moniker, const std::wstring &host,
           const std::wstring &fixture, const std::wstring &runtime) {
  Sid sid = DeriveSid(moniker);
  const std::wstring temporary = BottieProfileTempPath(ProfilePath(sid.Get()));
  Execution execution =
      Launch(moniker, host, {L"contained-probe", fixture, runtime, temporary});
  execution.standard_input.Reset();
  std::cout << Complete(execution);
}
} // namespace
int wmain(int argument_count, wchar_t **arguments) {
  try {
    if (argument_count == 3 && std::wstring_view(arguments[1]) == L"prepare")
      Prepare(arguments[2]);
    else if (argument_count == 3 &&
             std::wstring_view(arguments[1]) == L"cleanup")
      Cleanup(arguments[2]);
    else if (argument_count == 5 &&
             std::wstring_view(arguments[1]) == L"execute")
      Execute(arguments[2], arguments[3], arguments[4]);
    else if (argument_count == 5 &&
             std::wstring_view(arguments[1]) == L"cancel")
      Cancel(arguments[2], arguments[3], arguments[4]);
    else if (argument_count == 5 &&
             std::wstring_view(arguments[1]) == L"start-and-exit")
      StartAndExit(arguments[2], arguments[3], arguments[4]);
    else if (argument_count == 6 && std::wstring_view(arguments[1]) == L"probe")
      Probe(arguments[2], arguments[3], arguments[4], arguments[5]);
    else if (argument_count == 5 &&
             std::wstring_view(arguments[1]) == L"contained-probe")
      ContainedProbe(arguments[2], arguments[3], arguments[4]);
    else
      return 2;
    return 0;
  } catch (const std::runtime_error &error) {
    std::cout << "{\"reason\":\"" << error.what()
              << "\",\"status\":\"failed\"}\n";
    return 1;
  } catch (...) {
    std::cout << "{\"reason\":\"native\",\"status\":\"failed\"}\n";
    return 1;
  }
}
