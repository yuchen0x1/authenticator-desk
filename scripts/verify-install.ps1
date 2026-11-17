# verify-install.ps1 - installation evidence for AUTHENTICATOR DESK.
#
# Usage: ./scripts/verify-install.ps1 -Installer <path-to-setup.exe>
#
# Verifies the packaged app installs, launches, keeps the vault across an
# uninstall/reinstall cycle, and uninstalls cleanly. Every step prints an
# explicit PASS/FAIL line so the output can be pasted into
# docs/acceptance-checklist.md as evidence.

param(
    [Parameter(Mandatory = $true)][string]$Installer
)

$ErrorActionPreference = "Stop"
$installerPath = Resolve-Path $Installer
$installDir = "$env:LOCALAPPDATA\AUTHENTICATOR DESK"
$exePath = Join-Path $installDir "authenticator-desk.exe"
$dataDir = "$env:APPDATA\com.authenticatordesk.app"
$vaultPath = Join-Path $dataDir "vault.sqlite3"

function Assert-Step($name, $scriptBlock) {
    $ok = & $scriptBlock
    if ($ok) { Write-Host "PASS $name" } else { Write-Host "FAIL $name" }
    return $ok
}

Write-Host "==> Installing $installerPath"
$process = Start-Process -FilePath $installerPath -ArgumentList "/S" -Wait -PassThru
Assert-Step "NSIS installer exits with code 0" { $process.ExitCode -eq 0 } | Out-Null
Assert-Step "app binary exists after install" { Test-Path $exePath } | Out-Null

Write-Host "==> First launch"
if (Test-Path $exePath) {
    Start-Process -FilePath $exePath
    Start-Sleep -Seconds 6
    $running = Get-Process "authenticator-desk" -ErrorAction SilentlyContinue
    Assert-Step "app process is running after launch" { $null -ne $running } | Out-Null
    if ($running) { Stop-Process -Name "authenticator-desk" -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 2
}
else {
    Write-Host "FAIL app binary missing; skipping launch checks"
}

Write-Host "==> Vault data preservation across reinstall"
if (Test-Path $vaultPath) {
    $before = (Get-FileHash -Path $vaultPath -Algorithm SHA256).Hash
    Write-Host "vault size: $((Get-Item $vaultPath).Length) bytes"
}
else {
    Write-Host "note: no vault exists yet; create one in the app before the preservation test"
}

Start-Process -FilePath $installerPath -ArgumentList "/S" -Wait -PassThru | Out-Null
if (Test-Path $vaultPath) {
    $after = (Get-FileHash -Path $vaultPath -Algorithm SHA256).Hash
    Assert-Step "vault file unchanged across reinstall" { $before -eq $after } | Out-Null
}
else {
    Write-Host "PARTIAL vault preservation not measurable (no vault created)"
}

Write-Host "==> Uninstall"
$uninstaller = Get-ChildItem -Path $installDir -Filter "Uninstall*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($null -ne $uninstaller) {
    Start-Process -FilePath $uninstaller.FullName -ArgumentList "/S" -Wait
}
Assert-Step "install directory removed" { -not (Test-Path $exePath) } | Out-Null
Assert-Step "vault data survives uninstall" { Test-Path $vaultPath } | Out-Null
