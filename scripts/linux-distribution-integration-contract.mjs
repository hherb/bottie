/** Pure contracts shared by Bottie's Linux distribution integration and its portable tests. */

export const LINUX_DISTRIBUTION_INTEGRATION_STAGES = Object.freeze({
  hostPreflight: "host-preflight",
  fixtureSetup: "fixture-setup",
  ephemeralKey: "ephemeral-key",
  verificationPolicy: "verification-policy",
  fixturePackage: "fixture-package",
  positiveSignature: "positive-signature",
  weakDigestControl: "weak-digest-control",
  cleanup: "cleanup",
});

const ALLOWED_INTEGRATION_STAGES = new Set(Object.values(LINUX_DISTRIBUTION_INTEGRATION_STAGES));

/** Returns dpkg-deb arguments that avoid Ubuntu's debsigs-incompatible zstd default. */
export function linuxDistributionFixtureBuildArguments(packageRoot, debPath) {
  return ["--root-owner-group", "-Zxz", "--build", packageRoot, debPath];
}

/** Returns a bounded failure line without retaining command output, identities, or paths. */
export function linuxDistributionIntegrationFailureMessage(stage) {
  const safeStage = ALLOWED_INTEGRATION_STAGES.has(stage) ? stage : "unknown";
  return `[bottie] credential-free Linux distribution integration failed at ${safeStage}.`;
}
