/** Pure validation helpers for Bottie's path-free release runtime assets and packaged documents. */

const SCHEMA_VERSION = 1;
const SHA256_PATTERN = /^[a-f0-9]{64}$/;

/** Requires reviewed runtime metadata whose current source files match the recorded hashes. */
export function acceptsRuntimeAssets(assets, sources, documents) {
  return Boolean(
    assets?.schemaVersion === SCHEMA_VERSION &&
    isSha256(assets.manifestSha256) &&
    isSha256(assets.onnxRuntime?.licenceSha256) &&
    isSha256(assets.onnxRuntime?.thirdPartyNoticesSha256) &&
    assets.onnxRuntime.licenceSha256 === sources?.onnxRuntimeLicence &&
    assets.onnxRuntime.thirdPartyNoticesSha256 === sources?.onnxRuntimeNotices &&
    assets.embeddingGemma?.terms?.sha256 === sources?.modelNotice &&
    isSha256(documents?.licence) &&
    isSha256(documents?.notices) &&
    /^[a-f0-9]{40}$/.test(assets.embeddingGemma?.revision ?? "") &&
    isSha256(assets.embeddingGemma?.terms?.sha256),
  );
}

/** Accepts only explicit evidence bound to this exact model revision and reviewed terms notice. */
export function acceptsModelTerms(assets, evidence) {
  return Boolean(
    evidence?.schemaVersion === SCHEMA_VERSION &&
    evidence.accepted === true &&
    evidence.modelRevision === assets?.embeddingGemma?.revision &&
    evidence.termsSha256 === assets?.embeddingGemma?.terms?.sha256,
  );
}

/** Retains only the three required distributable document hashes. */
export function summarizeDocuments(documents) {
  return {
    licence: shaOrNull(documents?.licence),
    modelNotice: shaOrNull(documents?.modelNotice),
    thirdPartyNotices: shaOrNull(documents?.thirdPartyNotices),
  };
}

/** Requires package copies to match the current project licence, notice bundle, and reviewed model notice. */
export function acceptsPackagedDocuments(packaged, documents, runtimeAssets) {
  return Boolean(
    packaged?.licence === documents?.licence &&
    packaged.thirdPartyNotices === documents?.notices &&
    packaged.modelNotice === runtimeAssets?.embeddingGemma?.terms?.sha256,
  );
}

/** Returns a valid SHA-256 or null. */
function shaOrNull(value) {
  return isSha256(value) ? value : null;
}

/** Checks one lowercase SHA-256 value. */
function isSha256(value) {
  return typeof value === "string" && SHA256_PATTERN.test(value);
}
