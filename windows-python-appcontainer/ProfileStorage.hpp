#pragma once

#include <windows.h>

#include <array>
#include <string>

inline std::wstring BottieProfileLocalAppDataPath(const std::wstring &profile) {
  return profile + L"\\AC";
}

inline std::wstring BottieProfileTempPath(const std::wstring &profile) {
  return BottieProfileLocalAppDataPath(profile) + L"\\Temp";
}

inline bool BottieProfileTempIsReady(const std::wstring &profile) {
  const std::wstring path = BottieProfileTempPath(profile);
  const DWORD attributes = GetFileAttributesW(path.c_str());
  return attributes != INVALID_FILE_ATTRIBUTES &&
         (attributes & FILE_ATTRIBUTE_DIRECTORY) != 0;
}

struct BottieTemporaryStorageProbe {
  bool path_available = false;
  bool file_created = false;
  bool file_written = false;
  bool file_deleted = false;
  bool environment_matches_expected = false;
  bool environment_within_profile = false;
  bool path_matches_expected = false;
  bool path_within_profile = false;
  DWORD file_create_error = ERROR_SUCCESS;

  [[nodiscard]] bool Writable() const {
    return path_available && file_created && file_written && file_deleted;
  }
};

inline BottieTemporaryStorageProbe
ProbeBottieTemporaryStorage(const std::wstring &expected_path) {
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
  result.path_matches_expected =
      CompareStringOrdinal(resolved_path.c_str(), -1, expected_path.c_str(),
                           -1, TRUE) == CSTR_EQUAL;
  const std::wstring profile =
      expected_path.substr(0, expected_path.find_last_of(L"\\/"));
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
  result.environment_matches_expected =
      environment_length > 0 && environment_length < environment_path.size() &&
      CompareStringOrdinal(environment_path.data(), -1, expected_path.c_str(),
                           -1, TRUE) == CSTR_EQUAL;
  if (environment_length > 0 && environment_length < environment_path.size())
    result.environment_within_profile = within_profile(environment_path.data());

  const std::wstring file_path =
      expected_path + L"\\bottie-write-proof.tmp";
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
