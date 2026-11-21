# Verifies already-signed final artifacts. Does not sign or change files.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ExpectedThumbprint,
    [Parameter(Mandatory)][string[]]$Artifact
)
$ErrorActionPreference = 'Stop'
if ($ExpectedThumbprint -notmatch '^[a-fA-F0-9]{40}$') { throw 'A pinned publisher certificate thumbprint is required.' }
foreach ($path in $Artifact) {
    $item = Get-Item -LiteralPath $path
    if ($item -isnot [IO.FileInfo]) { throw 'Expected a signed artifact file.' }
    $signature = Get-AuthenticodeSignature -LiteralPath $item.FullName
    if ($signature.Status -ne 'Valid' -or !$signature.SignerCertificate -or
        $signature.SignerCertificate.Thumbprint -ne $ExpectedThumbprint -or
        !$signature.TimeStamperCertificate) {
        throw "Invalid publisher signature, certificate pin or timestamp: $($item.Name)"
    }
    Write-Host "PASS publisher and timestamp: $($item.Name)"
}
