import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

const severities = ["info", "low", "moderate", "high", "critical"];
const record = (value) => value !== null && typeof value === "object" && !Array.isArray(value);

export function verifyAuditReport(report) {
  if (!record(report) || "error" in report || !record(report.advisories) ||
      !record(report.metadata) || !record(report.metadata.vulnerabilities)) {
    throw new Error("Unrecognized or failed pnpm audit report; refusing to waive findings");
  }
  const counts = report.metadata.vulnerabilities;
  if (severities.some((severity) => !Number.isSafeInteger(counts[severity]) || counts[severity] < 0)) {
    throw new Error("Invalid audit vulnerability counts");
  }
  // No advisory exception is active. A new waiver must be separately reviewed,
  // version/path scoped, owned and time limited rather than inferred from an ID.
  if (Object.keys(report.advisories).length || severities.some((severity) => counts[severity] !== 0)) {
    throw new Error("Dependency advisories require review; no audit exception is active");
  }
  return true;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const path = process.argv[2];
  if (!path) throw new Error("Usage: verify-pnpm-audit.mjs <pnpm-audit.json>");
  verifyAuditReport(JSON.parse(await readFile(path, "utf8")));
  console.log("Verified audit report: no advisories and no active exceptions");
}
