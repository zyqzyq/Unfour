[CmdletBinding()]
param(
    [string]$Publisher = "CN=Unfour Local Test",
    [string]$OutputDirectory = "target\msix\certs",
    [int]$ValidYears = 2,
    [switch]$Remove
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw "MSIX test certificates can only be created on Windows."
}

$currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($currentIdentity)
$isAdministrator = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdministrator) {
    Write-Host "[msix-dev-cert] Administrator access is required for LocalMachine\TrustedPeople."
    $arguments = @(
        "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", ('"{0}"' -f $PSCommandPath),
        "-Publisher", ('"{0}"' -f $Publisher), "-OutputDirectory", ('"{0}"' -f $OutputDirectory),
        "-ValidYears", $ValidYears
    )
    if ($Remove) { $arguments += "-Remove" }
    $elevated = Start-Process -FilePath "pwsh.exe" -ArgumentList $arguments -Verb RunAs -Wait -PassThru
    exit $elevated.ExitCode
}

if ($Publisher -notmatch '^CN=') {
    throw "The local test Publisher must begin with 'CN='; found '$Publisher'."
}
if ($ValidYears -lt 1 -or $ValidYears -gt 5) {
    throw "ValidYears must be between 1 and 5."
}

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$outputRoot = if ([System.IO.Path]::IsPathRooted($OutputDirectory)) {
    [System.IO.Path]::GetFullPath($OutputDirectory)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repoRoot $OutputDirectory))
}
$allowedRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target\msix"))
if (-not $outputRoot.StartsWith($allowedRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to write a development certificate outside '$allowedRoot': $outputRoot"
}

$pfxPath = Join-Path $outputRoot "devcert.pfx"
$cerPath = Join-Path $outputRoot "devcert.cer"
$password = $env:MSIX_TEST_CERTIFICATE_PASSWORD
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null

function Remove-TrackedCertificate {
    param([Parameter(Mandatory = $true)][string]$Thumbprint)
    foreach ($storePath in @("Cert:\CurrentUser\TrustedPeople", "Cert:\LocalMachine\TrustedPeople")) {
        $certificatePath = Join-Path $storePath $Thumbprint
        if (Test-Path -LiteralPath $certificatePath) {
            Remove-Item -LiteralPath $certificatePath -Force
            Write-Host "[msix-dev-cert] Removed trusted certificate: $certificatePath"
        }
    }
}

if (Test-Path -LiteralPath $cerPath -PathType Leaf) {
    $oldCertificate = New-Object Security.Cryptography.X509Certificates.X509Certificate2($cerPath)
    Remove-TrackedCertificate -Thumbprint $oldCertificate.Thumbprint
    $oldCertificate.Dispose()
}
Remove-Item -LiteralPath $pfxPath -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $cerPath -Force -ErrorAction SilentlyContinue
if ($Remove) {
    Write-Host "[msix-dev-cert] Removed the tracked Unfour MSIX test certificate and local certificate files."
    exit 0
}

$securePassword = if ([string]::IsNullOrEmpty($password)) {
    New-Object System.Security.SecureString
} else {
    ConvertTo-SecureString -String $password -AsPlainText -Force
}
$certificate = New-SelfSignedCertificate `
    -Type Custom `
    -Subject $Publisher `
    -FriendlyName "Unfour MSIX Local Test" `
    -KeyUsage DigitalSignature `
    -KeyExportPolicy Exportable `
    -KeyAlgorithm RSA `
    -KeyLength 3072 `
    -HashAlgorithm SHA256 `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -NotAfter (Get-Date).AddYears($ValidYears) `
    -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")

try {
    Export-PfxCertificate -Cert $certificate -FilePath $pfxPath -Password $securePassword -Force | Out-Null
    Export-Certificate -Cert $certificate -FilePath $cerPath -Force | Out-Null
} finally {
    Remove-Item -LiteralPath "Cert:\CurrentUser\My\$($certificate.Thumbprint)" -Force -ErrorAction SilentlyContinue
}
$trusted = Import-Certificate -FilePath $cerPath -CertStoreLocation "Cert:\LocalMachine\TrustedPeople"
if ($null -eq $trusted -or $trusted.Thumbprint -cne $certificate.Thumbprint) {
    throw "The certificate was exported but could not be verified in LocalMachine\TrustedPeople."
}

Write-Host "[msix-dev-cert] Publisher: $Publisher"
Write-Host "[msix-dev-cert] Thumbprint: $($certificate.Thumbprint)"
Write-Host "[msix-dev-cert] PFX: $pfxPath"
Write-Host "[msix-dev-cert] CER: $cerPath"
