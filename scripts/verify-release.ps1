# verify-release.ps1 - pre-release evidence for AUTHENTICATOR DESK.
#
# Runs the full local verification gate and reports the bundle artifacts
# with SHA-256 hashes. It never signs or publishes anything.

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

$steps = @(
    @{ Name = "pnpm install --frozen-lockfile"; Command = { pnpm install --frozen-lockfile } },
    @{ Name = "pnpm exec tsc --noEmit"; Command = { pnpm exec tsc --noEmit } },
    @{ Name = "pnpm test -- --run"; Command = { pnpm test -- --run } },
    @{ Name = "cargo test"; Command = { cargo test --manifest-path src-tauri/Cargo.toml } },
    @{ Name = "pnpm tauri build"; Command = { pnpm tauri build } }
)

$results = @()
foreach ($step in $steps) {
    Write-Host "==> $($step.Name)"
    $output = & $step.Command 2>&1
    $exitCode = $LASTEXITCODE
    $results += [pscustomobject]@{
        Step = $step.Name
        ExitCode = $exitCode
        Status = if ($exitCode -eq 0) { "PASS" } else { "FAIL" }
    }
    if ($exitCode -ne 0) {
        Write-Host $output
        $results | Format-Table -AutoSize
        throw "$($step.Name) failed with exit code $exitCode"
    }
}

$bundleRoot = Join-Path $repoRoot "src-tauri/target/release/bundle"
Write-Host "==> Bundle artifacts"
Get-ChildItem -Path $bundleRoot -Recurse -Include *.exe, *.msi | ForEach-Object {
    $hash = (Get-FileHash -Path $_.FullName -Algorithm SHA256).Hash
    [pscustomobject]@{
        Artifact = $_.FullName.Substring($repoRoot.Length + 1)
        SizeBytes = $_.Length
        Sha256 = $hash
    }
} | Format-List

$results | Format-Table -AutoSize
Write-Host "Release verification gate completed."
