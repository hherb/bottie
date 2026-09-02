#pragma once

#include <windows.h>
#include <aclapi.h>
#include <sddl.h>

#include <array>
#include <cstddef>
#include <string>
#include <vector>

inline std::wstring BottieProfileLocalAppDataPath(const std::wstring &profile) {
  return profile + L"\\AC";
}

inline std::wstring BottieProfileTempPath(const std::wstring &profile) {
  return BottieProfileLocalAppDataPath(profile) + L"\\Temp";
}

inline bool ApplyBottieLowIntegrityLabel(std::wstring &path) {
  PSECURITY_DESCRIPTOR descriptor = nullptr;
  if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(
          L"S:(ML;OICI;NW;;;LW)", SDDL_REVISION_1, &descriptor, nullptr))
    return false;
  BOOL present = FALSE;
  BOOL defaulted = FALSE;
  PACL label = nullptr;
  const BOOL queried =
      GetSecurityDescriptorSacl(descriptor, &present, &label, &defaulted);
  DWORD applied = ERROR_INVALID_SECURITY_DESCR;
  if (queried && present && label != nullptr) {
    applied = SetNamedSecurityInfoW(path.data(), SE_FILE_OBJECT,
                                    LABEL_SECURITY_INFORMATION, nullptr,
                                    nullptr, nullptr, label);
  }
  LocalFree(descriptor);
  return applied == ERROR_SUCCESS;
}

// Grants temporary storage to the exact user and transient AppContainer pair.
inline bool PrepareBottieProfileTemp(const std::wstring &profile, PSID sid) {
  std::wstring path = BottieProfileTempPath(profile);
  if (!CreateDirectoryW(path.c_str(), nullptr) &&
      GetLastError() != ERROR_ALREADY_EXISTS)
    return false;

  PACL existing_acl = nullptr;
  PSECURITY_DESCRIPTOR descriptor = nullptr;
  const DWORD queried = GetNamedSecurityInfoW(
      path.data(), SE_FILE_OBJECT, DACL_SECURITY_INFORMATION, nullptr, nullptr,
      &existing_acl, nullptr, &descriptor);
  if (queried != ERROR_SUCCESS)
    return false;

  HANDLE current_token = nullptr;
  if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &current_token)) {
    LocalFree(descriptor);
    return false;
  }
  DWORD user_bytes = 0;
  GetTokenInformation(current_token, TokenUser, nullptr, 0, &user_bytes);
  if (GetLastError() != ERROR_INSUFFICIENT_BUFFER) {
    CloseHandle(current_token);
    LocalFree(descriptor);
    return false;
  }
  std::vector<std::byte> user_buffer(user_bytes);
  const BOOL user_queried = GetTokenInformation(
      current_token, TokenUser, user_buffer.data(), user_bytes, &user_bytes);
  CloseHandle(current_token);
  if (!user_queried) {
    LocalFree(descriptor);
    return false;
  }
  const auto *user = reinterpret_cast<const TOKEN_USER *>(user_buffer.data());

  constexpr DWORD kTemporaryRights = FILE_GENERIC_READ | FILE_GENERIC_WRITE |
                                     FILE_GENERIC_EXECUTE | DELETE |
                                     FILE_DELETE_CHILD;
  std::array<EXPLICIT_ACCESSW, 2> access{};
  for (auto &entry : access) {
    entry.grfAccessPermissions = kTemporaryRights;
    entry.grfAccessMode = GRANT_ACCESS;
    entry.grfInheritance = SUB_CONTAINERS_AND_OBJECTS_INHERIT;
  }
  BuildTrusteeWithSidW(&access[0].Trustee, user->User.Sid);
  BuildTrusteeWithSidW(&access[1].Trustee, sid);
  access[0].Trustee.TrusteeType = TRUSTEE_IS_USER;
  access[1].Trustee.TrusteeType = TRUSTEE_IS_GROUP;

  PACL updated_acl = nullptr;
  const DWORD combined = SetEntriesInAclW(
      static_cast<ULONG>(access.size()), access.data(), existing_acl,
      &updated_acl);
  DWORD applied = combined;
  if (combined == ERROR_SUCCESS) {
    applied = SetNamedSecurityInfoW(path.data(), SE_FILE_OBJECT,
                                    DACL_SECURITY_INFORMATION, nullptr,
                                    nullptr, updated_acl, nullptr);
  }
  if (updated_acl != nullptr)
    LocalFree(updated_acl);
  LocalFree(descriptor);
  return applied == ERROR_SUCCESS && ApplyBottieLowIntegrityLabel(path);
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
