# Local reproducible build/test gate. Never signs, publishes, or claims live-update acceptance.
[CmdletBinding()]
param(
    [switch]$InstallerSmoke,
    [ValidateSet('NSIS', 'MSI')][string]$InstallerFamily
)
$ErrorActionPreference = 'Stop'
if ($InstallerSmoke -and !$InstallerFamily) {
    throw 'Select exactly one -InstallerFamily NSIS or MSI per clean disposable VM or snapshot.'
}
Set-Location (Split-Path -Parent $PSScriptRoot)

$steps = @(
    @{ Name = 'Version agreement'; Command = { node scripts/release.mjs version } },
    @{ Name = 'Frozen frontend dependencies'; Command = { pnpm install --frozen-lockfile } },
    @{ Name = 'Frontend tests'; Command = { pnpm exec vitest run } },
    @{ Name = 'Node release policy tests'; Command = { node --test (Get-ChildItem scripts/*.test.mjs).FullName } },
    @{ Name = 'Frontend production build'; Command = { pnpm build } },
    @{ Name = 'Locked Rust core tests'; Command = { cargo test --locked --manifest-path src-tauri/Cargo.toml } },
    @{ Name = 'Signed updater and desktop tests'; Command = { cargo test --locked --manifest-path src-tauri/Cargo.toml --features signed-updater --lib } },
    @{ Name = 'Locked Windows packages (unsigned)'; Command = { pnpm tauri build --ci -- --locked } }
)
foreach ($step in $steps) {
    Write-Host "==> $($step.Name)"
    & $step.Command
    if ($LASTEXITCODE -ne 0) { throw "$($step.Name) failed with exit code $LASTEXITCODE" }
}

$bundles = @(
    @{ Family = 'NSIS'; Pattern = 'src-tauri/target/release/bundle/nsis/*.exe' },
    @{ Family = 'MSI'; Pattern = 'src-tauri/target/release/bundle/msi/*.msi' }
)
foreach ($bundle in $bundles) {
    $artifacts = @(Get-ChildItem -Path $bundle.Pattern -File)
    if ($artifacts.Count -ne 1) { throw 'Expected exactly one installer per family. Archive stale build output before verification.' }
    $artifact = $artifacts[0]
    [pscustomobject]@{ Artifact = $artifact.FullName; Bytes = $artifact.Length; Sha256 = (Get-FileHash -LiteralPath $artifact.FullName -Algorithm SHA256).Hash } | Format-List
    if ($InstallerSmoke -and $InstallerFamily -eq $bundle.Family) {
        # Never run both families in one VM: NSIS retains the path MSI discovers.
        & ./scripts/verify-install.ps1 -Installer $artifact.FullName -Family $bundle.Family
    } else {
        Write-Host "NOT RUN: $($bundle.Family) install lifecycle (select one -InstallerSmoke -InstallerFamily on a clean disposable VM)."
    }
}
Write-Host 'Local gate passed. NOT RUN: publisher signing, remote publication, genuine prior-version update, and Windows desktop acceptance.'
