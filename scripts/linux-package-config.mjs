/** Pure configuration helpers for Bottie's default and opt-in Python Linux package builds. */

import { join } from "node:path";

const PYTHON_DEVELOPMENT_CONFIG = "src-tauri/tauri.python-development.linux.conf.json";
const SMOKE_IDENTIFIER = "com.bottie.packaging-smoke";
const SMOKE_PRODUCT_NAME = "bottie-packaging-smoke";

/** Returns one locked, unsigned, non-interactive DEB build with the requested explicit overlays. */
function configuredLinuxBuildArguments({ python = false, smoke = false } = {}) {
  const arguments_ = ["build", "--bundles", "deb", "--no-sign", "--ci"];
  if (smoke) {
    arguments_.push("--config", JSON.stringify({ identifier: SMOKE_IDENTIFIER, productName: SMOKE_PRODUCT_NAME }));
  }
  if (python) arguments_.push("--config", PYTHON_DEVELOPMENT_CONFIG);
  return [...arguments_, "--", "--locked"];
}

/** Returns the exact locked, DEB-only Tauri arguments used by the default package command. */
export function linuxBuildArguments() {
  return configuredLinuxBuildArguments();
}

/** Returns a locked default build that isolates smoke storage under a distinct application identity. */
export function linuxSmokeBuildArguments() {
  return configuredLinuxBuildArguments({ smoke: true });
}

/** Returns the locked DEB-only build with the explicit development Python resources. */
export function linuxPythonBuildArguments() {
  return configuredLinuxBuildArguments({ python: true });
}

/** Returns the isolated smoke build with the same explicit Python resources as the real package. */
export function linuxPythonSmokeBuildArguments() {
  return configuredLinuxBuildArguments({ python: true, smoke: true });
}

/** Produces provider settings that can contact only the supplied isolated loopback endpoint. */
export function offlineProviderSettings(port) {
  return {
    omlxBaseUrl: `http://127.0.0.1:${port}/`,
    ollamaBaseUrl: `http://127.0.0.1:${port}/`,
    setupCompleted: true,
    lastProviderId: "omlx",
    lastModelId: "packaging-offline-smoke",
  };
}

/** Resolves the process-owned XDG roots and exact distinct-identity app paths used by smoke. */
export function smokeXdgDirectories(root) {
  return {
    cache: join(root, "cache"),
    config: join(root, "config"),
    data: join(root, "data"),
    runtime: join(root, "runtime"),
    support: join(root, "data", SMOKE_IDENTIFIER),
    settings: join(root, "config", SMOKE_IDENTIFIER, "providers.json"),
  };
}
