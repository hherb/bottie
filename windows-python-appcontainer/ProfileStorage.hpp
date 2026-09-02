#pragma once

#include <windows.h>

#include <array>
#include <string>
#include <vector>

inline std::wstring BottieProfileLocalAppDataPath(const std::wstring &profile) {
  return profile + L"\\AC";
}

inline std::wstring BottieProfileTempPath(const std::wstring &profile) {
  return BottieProfileLocalAppDataPath(profile) + L"\\Temp";
}

inline bool EnsureBottieProfileDirectory(const std::wstring &path) {
  if (!CreateDirectoryW(path.c_str(), nullptr) &&
      GetLastError() != ERROR_ALREADY_EXISTS)
    return false;
  const DWORD attributes = GetFileAttributesW(path.c_str());
  return attributes != INVALID_FILE_ATTRIBUTES &&
         (attributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
}

inline bool PrepareBottieProfileStorage(const std::wstring &profile) {
  return EnsureBottieProfileDirectory(BottieProfileLocalAppDataPath(profile)) &&
         EnsureBottieProfileDirectory(BottieProfileTempPath(profile));
}

inline std::vector<wchar_t>
BottieMinimalEnvironment(const std::wstring &profile) {
  const std::wstring local = BottieProfileLocalAppDataPath(profile);
  const std::wstring temporary = BottieProfileTempPath(profile);
  std::wstring block = L"LOCALAPPDATA=" + local;
  block.push_back(L'\0');
  block.append(L"TEMP=").append(temporary).push_back(L'\0');
  block.append(L"TMP=").append(temporary).push_back(L'\0');
  block.push_back(L'\0');
  return {block.begin(), block.end()};
}

inline bool EnsureBottieResolvedDirectory(const std::wstring &profile,
                                           const std::wstring &path) {
  std::size_t separator = path.find_first_of(L"\\/", profile.size() + 1);
  while (separator != std::wstring::npos) {
    if (!EnsureBottieProfileDirectory(path.substr(0, separator)))
      return false;
    separator = path.find_first_of(L"\\/", separator + 1);
  }
  return EnsureBottieProfileDirectory(path);
}

struct BottieTemporaryStorageProbe {
  bool directory_prepared = false;
  bool path_available = false;
  bool file_created = false;
  bool file_written = false;
  bool file_deleted = false;
  bool environment_matches_path = false;
  bool environment_within_profile = false;
  bool path_within_profile = false;
  DWORD file_create_error = ERROR_SUCCESS;

  [[nodiscard]] bool Writable() const {
    return path_available && directory_prepared && file_created &&
           file_written && file_deleted;
  }
};

inline BottieTemporaryStorageProbe
ProbeBottieTemporaryStorage(const std::wstring &profile) {
  BottieTemporaryStorageProbe result;
  std::array<wchar_t, MAX_PATH> temporary_path{};
  const DWORD length = GetTempPathW(static_cast<DWORD>(temporary_path.size()),
                                    temporary_path.data());
  result.path_available = length > 0 && length < temporary_path.size();
  if (!result.path_available)
    return result;

  std::wstring resolved_path(temporary_path.data());
  while (!resolved_path.empty() &&
         (resolved_path.back() == L'\\' || resolved_path.back() == L'/'))
    resolved_path.pop_back();
  const auto within_profile = [&profile](const std::wstring &candidate) {
    return candidate.size() > profile.size() &&
           CompareStringOrdinal(candidate.c_str(),
                                static_cast<int>(profile.size()),
                                profile.c_str(), static_cast<int>(profile.size()),
                                TRUE) == CSTR_EQUAL &&
           (candidate[profile.size()] == L'\\' ||
            candidate[profile.size()] == L'/');
  };
  result.path_within_profile = within_profile(resolved_path);
  std::array<wchar_t, MAX_PATH> environment_path{};
  const DWORD environment_length = GetEnvironmentVariableW(
      L"TMP", environment_path.data(),
      static_cast<DWORD>(environment_path.size()));
  if (environment_length > 0 && environment_length < environment_path.size()) {
    std::wstring environment(environment_path.data());
    while (!environment.empty() &&
           (environment.back() == L'\\' || environment.back() == L'/'))
      environment.pop_back();
    result.environment_matches_path =
        CompareStringOrdinal(environment.c_str(), -1, resolved_path.c_str(),
                             -1, TRUE) == CSTR_EQUAL;
    result.environment_within_profile = within_profile(environment);
  }

  result.directory_prepared =
      result.environment_matches_path && result.environment_within_profile &&
      result.path_within_profile &&
      EnsureBottieResolvedDirectory(profile, resolved_path);
  if (!result.directory_prepared) {
    result.file_create_error = GetLastError();
    return result;
  }

  const std::wstring file_path =
      resolved_path + L"\\bottie-write-proof.tmp";
  HANDLE file = CreateFileW(file_path.c_str(), GENERIC_WRITE, 0, nullptr,
                            CREATE_ALWAYS, FILE_ATTRIBUTE_TEMPORARY, nullptr);
  result.file_created = file != INVALID_HANDLE_VALUE;
  if (!result.file_created) {
    result.file_create_error = GetLastError();
    return result;
  }

  constexpr char kProbeByte = 'B';
  DWORD written = 0;
  result.file_written =
      WriteFile(file, &kProbeByte, 1, &written, nullptr) && written == 1;
  CloseHandle(file);
  result.file_deleted = DeleteFileW(file_path.c_str());
  return result;
}
