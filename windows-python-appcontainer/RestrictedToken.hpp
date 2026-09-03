#pragma once

#include <windows.h>

#include <cstddef>
#include <vector>

/** Creates a primary token with every removable privilege disabled. */
inline HANDLE CreateBottieRestrictedToken() noexcept {
  HANDLE current = nullptr;
  if (!OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY, &current))
    return nullptr;
  HANDLE result = nullptr;
  const BOOL created = CreateRestrictedToken(current, DISABLE_MAX_PRIVILEGE, 0,
                                             nullptr, 0, nullptr, 0, nullptr, &result);
  CloseHandle(current);
  return created ? result : nullptr;
}

/** Confirms only Windows' non-removable directory-traverse privilege may remain enabled. */
inline bool BottieTokenPrivilegesStripped(HANDLE token) noexcept {
  DWORD bytes = 0;
  GetTokenInformation(token, TokenPrivileges, nullptr, 0, &bytes);
  if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
    return false;
  std::vector<std::byte> buffer(bytes);
  if (!GetTokenInformation(token, TokenPrivileges, buffer.data(), bytes, &bytes))
    return false;
  LUID traverse{};
  if (!LookupPrivilegeValueW(nullptr, SE_CHANGE_NOTIFY_NAME, &traverse))
    return false;
  const auto *privileges = reinterpret_cast<const TOKEN_PRIVILEGES *>(buffer.data());
  for (DWORD index = 0; index < privileges->PrivilegeCount; ++index) {
    const auto &privilege = privileges->Privileges[index];
    const bool enabled = (privilege.Attributes & SE_PRIVILEGE_ENABLED) != 0;
    const bool is_traverse = privilege.Luid.LowPart == traverse.LowPart &&
                             privilege.Luid.HighPart == traverse.HighPart;
    if (enabled && !is_traverse)
      return false;
  }
  return true;
}

/** Confirms the AppContainer launch produced a Low integrity token. */
inline bool BottieTokenIsLowIntegrity(HANDLE token) noexcept {
  DWORD bytes = 0;
  GetTokenInformation(token, TokenIntegrityLevel, nullptr, 0, &bytes);
  if (GetLastError() != ERROR_INSUFFICIENT_BUFFER)
    return false;
  std::vector<std::byte> buffer(bytes);
  if (!GetTokenInformation(token, TokenIntegrityLevel, buffer.data(), bytes,
                           &bytes))
    return false;
  const auto *label =
      reinterpret_cast<const TOKEN_MANDATORY_LABEL *>(buffer.data());
  const UCHAR count = *GetSidSubAuthorityCount(label->Label.Sid);
  if (count == 0)
    return false;
  return *GetSidSubAuthority(label->Label.Sid, count - 1) ==
         SECURITY_MANDATORY_LOW_RID;
}
