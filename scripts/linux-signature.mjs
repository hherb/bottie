/** Pure structural helpers for Linux DEB signature evidence. */

/** Classifies embedded signature members without claiming cryptographic verification. */
export function classifyDebianSignatureMembers(members) {
  const identified = members.some((member) => member.startsWith("_gpg"));
  return { classification: identified ? "identified" : "unsigned", verifies: false };
}
