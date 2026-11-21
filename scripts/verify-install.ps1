# Runs a trusted installer and uninstalls it. Use only a disposable Windows VM or runner.
# The MSI registers per-machine and requires an elevated session. The custom path does not
# isolate Windows Installer registration, shortcuts, or other system-level changes.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Installer,
    [ValidateSet('NSIS', 'MSI')][string]$Family
)
$ErrorActionPreference = 'Stop'

$installerFile = Get-Item -LiteralPath $Installer
if ($installerFile -isnot [IO.FileInfo]) { throw 'Expected an installer file.' }
$detectedFamily = switch ($installerFile.Extension) {
    '.exe' { 'NSIS' }
    '.msi' { 'MSI' }
    default { throw 'Expected an .exe (NSIS) or .msi installer.' }
}
if ($Family -and $Family -ne $detectedFamily) {
    throw "Installer extension does not match -Family $Family."
}
$Family = $detectedFamily
$installerPath = $installerFile.FullName
$exeName = 'authenticator-desk.exe'

function Test-ExistingInstallation {
    $productName = 'AUTHENTICATOR DESK'
    $uninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall'
    $productKey = 'Software\authenticatordesk\AUTHENTICATOR DESK'
    # Examine both scopes and registry views, even from a 32-bit PowerShell host.
    foreach ($hive in 'CurrentUser', 'LocalMachine') {
        foreach ($view in 'Registry64', 'Registry32') {
            $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($hive, $view)
            try {
                $product = $base.OpenSubKey($productKey)
                if ($product) {
                    try {
                        # WiX searches both values for a previous install directory.
                        if ($product.GetValue('') -or $product.GetValue('InstallDir')) { return $true }
                    } finally { $product.Dispose() }
                }

                $uninstall = $base.OpenSubKey($uninstallKey)
                if ($uninstall) {
                    try {
                        foreach ($name in $uninstall.GetSubKeyNames()) {
                            if ($name -eq $productName) { return $true }
                            $entry = $uninstall.OpenSubKey($name)
                            if ($entry) {
                                try {
                                    if ($entry.GetValue('DisplayName') -eq $productName) { return $true }
                                } finally { $entry.Dispose() }
                            }
                        }
                    } finally { $uninstall.Dispose() }
                }
            } finally { $base.Dispose() }
        }
    }
    return $false
}

if ((Get-Process -Name 'authenticator-desk' -ErrorAction SilentlyContinue) -or (Test-ExistingInstallation)) {
    throw 'An AUTHENTICATOR DESK installation, remembered install path, or process exists. Use a clean disposable Windows VM or runner.'
}

if ($Family -eq 'MSI') {
    $wixRoot = Join-Path (Split-Path -Parent $PSScriptRoot) 'src-tauri/target/release/wix'
    if (Test-Path -LiteralPath $wixRoot -PathType Container) {
        foreach ($source in Get-ChildItem -LiteralPath $wixRoot -Filter 'main.wxs' -File -Recurse) {
            [xml]$wix = Get-Content -LiteralPath $source.FullName -Raw
            $ns = [Xml.XmlNamespaceManager]::new($wix.NameTable)
            $ns.AddNamespace('w', $wix.DocumentElement.NamespaceURI)
            $files = $wix.SelectNodes('//w:DirectoryRef[@Id="INSTALLDIR"]//w:File | //w:Directory[@Id="INSTALLDIR"]//w:File', $ns)
            $appFile = $files | Where-Object { [IO.Path]::GetFileName($_.GetAttribute('Source')) -eq $exeName }
            if (!$appFile) { throw "Generated WiX source does not place $exeName under INSTALLDIR: $($source.FullName)" }
            Write-Host "PASS generated WiX uses INSTALLDIR for $exeName"
        }
    }
}

$root = Join-Path ([IO.Path]::GetTempPath()) ('authdesk-smoke-' + [Guid]::NewGuid())
$installDir = Join-Path $root 'app'
$exe = Join-Path $installDir $exeName
$vaultDir = Join-Path $root 'data'
$marker = Join-Path $vaultDir 'preservation-marker.txt'
$oldAppData = $env:AUTHDESK_DATA_DIR
$oldWebViewData = $env:WEBVIEW2_USER_DATA_FOLDER
$process = $null

function Invoke-Msi([string]$Phase) {
    $log = Join-Path $root "$Phase-msi.log"
    $operation = if ($Phase -eq 'uninstall') { '/x' } else { '/i' }
    $commandLine = "$operation `"$installerPath`" /qn /norestart"
    if ($operation -eq '/i') { $commandLine += " INSTALLDIR=`"$installDir`"" }
    # A second plain /i of the same package is maintenance, not a forced reinstall.
    if ($Phase -eq 'reinstall') { $commandLine += ' REINSTALL=ALL REINSTALLMODE=amus' }
    $commandLine += " /l*v `"$log`""
    $msiexec = Join-Path ([Environment]::SystemDirectory) 'msiexec.exe'
    $result = Start-Process -FilePath $msiexec -ArgumentList $commandLine -WindowStyle Hidden -PassThru -Wait
    if ($result.ExitCode -notin 0, 3010) {
        throw "MSI $Phase failed (exit $($result.ExitCode)). Log: $log"
    }
    if ($result.ExitCode -eq 3010) {
        Write-Warning "MSI $Phase requires a restart (3010); continuing without restarting. Log: $log"
    }
}

function Invoke-Setup([string]$Phase) {
    if ($Family -eq 'MSI') {
        Invoke-Msi $Phase
    } else {
        # NSIS requires unquoted /D=... as the last argument, including paths with spaces.
        $setup = Start-Process -FilePath $installerPath -ArgumentList "/S /D=$installDir" -WindowStyle Hidden -PassThru -Wait
        if ($setup.ExitCode -ne 0) { throw "NSIS $Phase failed (exit $($setup.ExitCode))" }
    }
    if (!(Test-Path -LiteralPath $exe -PathType Leaf)) {
        throw "$Family $Phase did not place the executable at $exe. Smoke evidence: $root"
    }
}

function Assert-DataPreserved([string]$Phase) {
    if (!(Test-Path -LiteralPath $marker -PathType Leaf) -or
        (Get-FileHash -LiteralPath $marker).Hash -ne $digest) {
        throw "$Family $Phase removed or changed the external data marker: $marker"
    }
}

try {
    New-Item -ItemType Directory -Path $vaultDir -Force | Out-Null
    Set-Content -LiteralPath $marker -Value 'Synthetic install test'
    $digest = (Get-FileHash -LiteralPath $marker).Hash
    $env:AUTHDESK_DATA_DIR = $vaultDir
    $env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $root 'webview2'
    Write-Host "Smoke evidence directory: $root"

    Invoke-Setup 'install'
    Write-Host "PASS $Family install"
    $process = Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 8
    $process.Refresh()
    if ($process.HasExited) { throw "Installed app exited during startup (exit $($process.ExitCode))" }
    Write-Host 'PASS installed process remains running'
    Stop-Process -InputObject $process
    $process.WaitForExit()

    Invoke-Setup 'reinstall'
    Assert-DataPreserved 'reinstall'
    Write-Host "PASS $Family reinstall preserves external app data"

    if ($Family -eq 'MSI') {
        Invoke-Msi 'uninstall'
    } else {
        $uninstaller = Join-Path $installDir 'uninstall.exe'
        if (!(Test-Path -LiteralPath $uninstaller -PathType Leaf)) { throw 'NSIS uninstaller missing' }
        $result = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -Wait -PassThru
        if ($result.ExitCode -ne 0) { throw "NSIS uninstall failed (exit $($result.ExitCode))" }
    }
    if (Test-Path -LiteralPath $exe) {
        throw "$Family uninstall left the executable at $exe. Smoke evidence: $root"
    }
    Assert-DataPreserved 'uninstall'
    Write-Host "PASS $Family uninstall preserves external app data"
} finally {
    $env:AUTHDESK_DATA_DIR = $oldAppData
    $env:WEBVIEW2_USER_DATA_FOLDER = $oldWebViewData
    if ($process -and !$process.HasExited) { Stop-Process -InputObject $process }
}
