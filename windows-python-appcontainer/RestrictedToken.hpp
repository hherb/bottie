#pragma once

#include <windows.h>

#include <array>
#include <cstddef>

/** Creates a privilege-stripped token restricted to the caller and ordinary system-user resources. */
inline HANDLE CreateBottieRestrictedToken() noexcept {
  HANDLE current = nullptr;
  if (!OpenProcessToken(GetCurrentProcess(), TOKEN_DUPLICATE | TOKEN_QUERY, &current))
    return nullptr;

  std::array<std::byte, sizeof(TOKEN_USER) + SECURITY_MAX_SID_SIZE> user_bytes{};
  std::array<std::byte, SECURITY_MAX_SID_SIZE> users_bytes{};
  DWORD returned = 0;
  DWORD users_size = static_cast<DWORD>(users_bytes.size());
  const BOOL user_loaded = GetTokenInformation(current, TokenUser, user_bytes.data(),
                                               static_cast<DWORD>(user_bytes.size()), &returned);
  const BOOL users_created =
      CreateWellKnownSid(WinBuiltinUsersSid, nullptr, users_bytes.data(), &users_size);
  if (!user_loaded || !users_created) {
    CloseHandle(current);
    return nullptr;
  }

  const auto *user = reinterpret_cast<const TOKEN_USER *>(user_bytes.data());
  std::array<SID_AND_ATTRIBUTES, 2> restricted{
      SID_AND_ATTRIBUTES{user->User.Sid, 0},
      SID_AND_ATTRIBUTES{users_bytes.data(), 0},
  };
  HANDLE result = nullptr;
  const BOOL created = CreateRestrictedToken(current, DISABLE_MAX_PRIVILEGE, 0, nullptr, 0, nullptr,
                                             static_cast<DWORD>(restricted.size()), restricted.data(), &result);
  CloseHandle(current);
  return created ? result : nullptr;
}
