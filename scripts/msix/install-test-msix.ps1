[CmdletBinding()]
param([string]$MsixPath)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw "MSIX installation can only run on Windows."
}

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$outputRoot = Join-Path $repoRoot "target\msix"
if ([string]::IsNullOrWhiteSpace($MsixPath)) {
    $package = Get-ChildItem -LiteralPath $outputRoot -Filter "*_TEST-SIGNED.msix" -File -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if ($null -eq $package) {
        throw "No signed local test MSIX exists under '$outputRoot'. Run 'pnpm run msix:setup' first."
    }
    $MsixPath = $package.FullName
} else {
    $MsixPath = [System.IO.Path]::GetFullPath($MsixPath)
}

if (-not (Test-Path -LiteralPath $MsixPath -PathType Leaf)) {
    throw "MSIX package was not found: $MsixPath"
}
$signature = Get-AuthenticodeSignature -FilePath $MsixPath
if ($signature.Status -ne "Valid") {
    throw "MSIX signature status is '$($signature.Status)'. Run 'pnpm run msix:setup' and approve its UAC trust prompt before installing. $($signature.StatusMessage)"
}

Add-AppxPackage -Path $MsixPath
Write-Host "[msix-install] Installed: $MsixPath"
Write-Host "[msix-install] Start menu app: Unfour (MSIX Test)"
Write-Host "[msix-install] MCP command: unfour-mcp.exe"
