#pragma once

#include <windows.h>
#include <aclapi.h>
#include <sddl.h>

#include <string>

inline std::wstring BottieProfileTempPath(const std::wstring &profile) {
  return profile + L"\\bottie-temp";
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

// Gives only the transient AppContainer identity a writable profile directory.
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
  return applied == ERROR_SUCCESS && ApplyBottieLowIntegrityLabel(path);
}
