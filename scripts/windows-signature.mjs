/** Reduces Windows Authenticode inspection to path- and identity-free policy evidence. */

/** Classifies Authenticode status without retaining certificate or publisher identities. */
export function classifyAuthenticodeStatus(status) {
  if (status === "NotSigned") return "unsigned";
  if (status === "Valid") return "identified";
  return "untrusted";
}

/** Retains only verification and timestamp state from one structured PowerShell result. */
export function parseAuthenticodeEvidence(output) {
  let result;
  try {
    result = JSON.parse(output);
  } catch {
    throw new Error("Windows did not return structured Authenticode evidence.");
  }
  if (!result || typeof result.status !== "string" || typeof result.timestamped !== "boolean") {
    throw new Error("Windows returned incomplete structured Authenticode evidence.");
  }
  return {
    classification: classifyAuthenticodeStatus(result.status),
    timestamped: result.timestamped,
    verifies: result.status === "Valid",
  };
}
