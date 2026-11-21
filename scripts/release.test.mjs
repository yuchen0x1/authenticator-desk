import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { readFile } from "node:fs/promises";
import { assertStablePromotion } from "./release.mjs";

describe("stable release promotion",()=>{
  it("allows an initial stable version or a strictly newer stable version",()=>{
    assert.doesNotThrow(()=>assertStablePromotion("v0.2.1",""));
    assert.doesNotThrow(()=>assertStablePromotion("v0.2.1","v0.1.0\nv0.2.0"));
    assert.doesNotThrow(()=>assertStablePromotion("v1.0.0","v0.99.99"));
    assert.doesNotThrow(()=>assertStablePromotion("v9007199254740994.0.0","v9007199254740993.0.0"));
  });
  it("rejects an equal/older version, RC promotion and malformed metadata",()=>{
    for(const [candidate,previous] of [["v0.2.0","v0.2.0"],["v0.2.0","v0.2.1"],["v1.0.0-rc.1","v0.2.0"],["v01.2.3",""],["v1.2.3","unknown"]]) {
      assert.throws(()=>assertStablePromotion(candidate,previous));
    }
  });
});

describe("release policy wiring",()=>{
  it("binds release creation to the verified tag commit instead of a moving default branch", async () => {
    const release = await readFile(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
    assert.match(release, /gh release create "\$GITHUB_REF_NAME" --verify-tag --target "\$GITHUB_SHA"/);
  });
  it("requires security and authorization before signed verification and publication",async()=>{
    const release=await readFile(new URL("../.github/workflows/release.yml",import.meta.url),"utf8");
    const ci=await readFile(new URL("../.github/workflows/ci.yml",import.meta.url),"utf8");
    assert.match(release,/group: release-promotion/);
    assert.match(release,/needs: \[tag, security\]/);
    assert.match(release,/uses: \.\/\.github\/workflows\/security.yml/);
    assert.match(ci,/windows:\s+needs: authorize/);
    assert.match(ci,/if: inputs.signed\s+run: node scripts\/release.mjs context/);
    assert.match(release,/node scripts\/release.mjs promotion/);
    assert.match(release,/verify published-download/);
    assert.match(release,/cmp latest-update.json published-download\/update.json/);
  });
  it("isolates both installer lifecycles on fresh runners without bypassing safety checks", async () => {
    const ci = await readFile(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
    const installerScript = await readFile(new URL("./verify-install.ps1", import.meta.url), "utf8");
    const installers = ci.slice(ci.indexOf("\n  installers:"));
    const build = ci.slice(ci.indexOf("\n  windows:"), ci.indexOf("\n  installers:"));
    assert.match(installers, /needs: windows/);
    assert.match(installers, /runs-on: windows-2025/);
    assert.match(installers, /fail-fast: false/);
    assert.match(installers, /family: NSIS\s+extension: exe/);
    assert.match(installers, /family: MSI\s+extension: msi/);
    assert.match(installers, /actions\/download-artifact@[a-f0-9]{40}/);
    assert.match(installers, /name: windows-x64/);
    assert.match(installers, /verify-install\.ps1 -Installer/);
    assert.doesNotMatch(build, /verify-install\.ps1/);
    assert.doesNotMatch(installerScript, /GITHUB_ACTIONS/);
    assert.match(installerScript, /Get-Process -Name 'authenticator-desk'/);
    assert.match(installerScript, /-or \(Test-ExistingInstallation\)/);
  });
  it("requires one explicit installer family for a local disposable-VM smoke run", async () => {
    const script = await readFile(new URL("./verify-release.ps1", import.meta.url), "utf8");
    assert.match(script, /\[ValidateSet\('NSIS', 'MSI'\)\]\[string\]\$InstallerFamily/);
    assert.match(script, /if \(\$InstallerSmoke -and !\$InstallerFamily\)/);
    assert.match(script, /if \(\$InstallerSmoke -and \$InstallerFamily -eq \$bundle\.Family\)/);
    assert.match(script, /verify-install\.ps1 -Installer \$artifact\.FullName -Family \$bundle\.Family/);
  });
  it("executes feature-gated desktop contracts instead of only compiling them", async () => {
    const ci = await readFile(new URL("../.github/workflows/ci.yml", import.meta.url), "utf8");
    assert.match(ci, /run: cargo \+1\.99\.1 test --locked --manifest-path src-tauri\/Cargo\.toml --features desktop --test desktop/);
  });
  it("requires cryptographic verification in the actual signed verifier",async()=>{
    const script=await readFile(new URL("./release.mjs",import.meta.url),"utf8");
    assert.match(script,/await verifyReleaseSignatures\(dir, tag, publicKey\)/);
    assert.match(script,/64 \* 1024 \* 1024/);
    assert.match(script,/96 \* 1024/);
    assert.match(script,/async function buildSigned\(\) \{\s+requireTrustedReleaseInvocation\(\)/);
  });
});
