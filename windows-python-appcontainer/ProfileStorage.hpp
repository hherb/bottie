#pragma once

#include <windows.h>
#include <aclapi.h>

#include <string>

inline std::wstring BottieProfileTempPath(const std::wstring &profile) {
  return profile + L"\\bottie-temp";
}

// Gives only the transient AppContainer identity a writable profile directory.
inline bool PrepareBottieProfileTemp(const std::wstring &profile, PSID sid) {
  const std::wstring path = BottieProfileTempPath(profile);
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

  EXPLICIT_ACCESSW access{};
  access.grfAccessPermissions = FILE_GENERIC_READ | FILE_GENERIC_WRITE |
                                FILE_GENERIC_EXECUTE | DELETE |
                                FILE_DELETE_CHILD;
  access.grfAccessMode = GRANT_ACCESS;
  access.grfInheritance = SUB_CONTAINERS_AND_OBJECTS_INHERIT;
  BuildTrusteeWithSidW(&access.Trustee, sid);

  PACL updated_acl = nullptr;
  const DWORD combined =
      SetEntriesInAclW(1, &access, existing_acl, &updated_acl);
  DWORD applied = combined;
  if (combined == ERROR_SUCCESS) {
    applied = SetNamedSecurityInfoW(path.data(), SE_FILE_OBJECT,
                                    DACL_SECURITY_INFORMATION, nullptr,
                                    nullptr, updated_acl, nullptr);
  }
  if (updated_acl != nullptr)
    LocalFree(updated_acl);
  LocalFree(descriptor);
  return applied == ERROR_SUCCESS;
}
