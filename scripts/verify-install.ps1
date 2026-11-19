# This script installs and uninstalls the application. Run on a disposable Windows user or runner.
param([Parameter(Mandatory)][string]$Installer)
$ErrorActionPreference = "Stop"

$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$root = Join-Path ([IO.Path]::GetTempPath()) ("authdesk-smoke-" + [Guid]::NewGuid())
$installDir = Join-Path $root "app"
$exe = Join-Path $installDir "authenticator-desk.exe"
$vaultDir = Join-Path $root "data"
$marker = Join-Path $vaultDir "preservation-marker.txt"
$oldAppData = $env:AUTHDESK_DATA_DIR
$process = $null

function Invoke-Setup {
    $setup = Start-Process -FilePath $installerPath -ArgumentList "/S /D=$installDir" -WindowStyle Hidden -PassThru -Wait
    if ($setup.ExitCode -ne 0 -or !(Test-Path -LiteralPath $exe)) {
        throw "NSIS installation failed (exit $($setup.ExitCode))"
    }
}

if ($env:GITHUB_ACTIONS -ne "true") {
    $registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall'
    $installed = Get-ItemProperty "$registry\*" -ErrorAction SilentlyContinue |
        Where-Object DisplayName -EQ 'AUTHENTICATOR DESK'
    if ($installed -or (Get-Process authenticator-desk -ErrorAction SilentlyContinue)) {
        throw "An AUTHENTICATOR DESK installation or process exists. Use a disposable Windows user."
    }
}

try {
    New-Item -ItemType Directory -Path $vaultDir -Force | Out-Null
    Set-Content -LiteralPath $marker -Value "Synthetic install test"
    $digest = (Get-FileHash -LiteralPath $marker).Hash
    $env:AUTHDESK_DATA_DIR = $vaultDir

    Invoke-Setup
    Write-Host "PASS NSIS install"
    $process = Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 8
    $process.Refresh()
    if ($process.HasExited) { throw "Installed app exited during startup (exit $($process.ExitCode))" }
    Write-Host "PASS installed process remains running"
    Stop-Process -Id $process.Id
    $process.WaitForExit()

    Invoke-Setup
    if ((Get-FileHash -LiteralPath $marker).Hash -ne $digest) { throw "Reinstall changed the data marker" }
    Write-Host "PASS NSIS reinstall preserves external app data"

    $uninstaller = Get-ChildItem -LiteralPath $installDir -Filter "uninstall.exe" | Select-Object -First 1
    if (!$uninstaller) { throw "Uninstaller missing" }
    $uninstall = Start-Process -FilePath $uninstaller.FullName -ArgumentList "/S" -WindowStyle Hidden -Wait -PassThru
    if ($uninstall.ExitCode -ne 0 -or (Test-Path -LiteralPath $exe)) {
        throw "NSIS uninstall failed (exit $($uninstall.ExitCode))"
    }
    if ((Get-FileHash -LiteralPath $marker).Hash -ne $digest) { throw "Uninstall changed the data marker" }
    Write-Host "PASS NSIS uninstall preserves external app data"
} finally {
    $env:AUTHDESK_DATA_DIR = $oldAppData
    if ($process -and !$process.HasExited) { Stop-Process -Id $process.Id }
}
