import { readFile } from "node:fs/promises";

const reportPath = process.argv[2];
if (!reportPath) throw new Error("Usage: verify-pnpm-audit.mjs <pnpm-audit.json>");

const report = JSON.parse(await readFile(reportPath, "utf8"));
if (report.error) throw new Error(report.error.message ?? "pnpm audit failed");

const advisories = Object.values(report.advisories ?? {});
const knownUpstreamAdvisory = "GHSA-fc5q-39xc-vjpf";
const unexpected = advisories.filter((advisory) =>
  (advisory.github_advisory_id ?? advisory.github_advisory?.ghsa_id) !== knownUpstreamAdvisory
);

if (unexpected.length > 0) {
  console.error(JSON.stringify(unexpected, null, 2));
  process.exit(1);
}

if (advisories.length > 0) {
  console.warn("The development toolchain still reports GHSA-fc5q-39xc-vjpf for picomatch.");
  console.warn("npm does not publish a fixed picomatch version yet. Production audit passed separately.");
}
