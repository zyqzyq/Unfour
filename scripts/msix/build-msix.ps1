[CmdletBinding()]
param(
    [ValidateSet("Store", "Test")]
    [string]$PackageChannel = "Test",
    [string]$ConfigPath,
    [string]$IdentityName,
    [string]$Publisher,
    [string]$PublisherDisplayName,
    [string]$DisplayName,
    [string]$Description,
    [string]$Version,
    [ValidateSet("x64")]
    [string]$Architecture,
    [ValidateSet("None", "Pfx")]
    [string]$SignMode = "None",
    [string]$PfxPath,
    [string]$PfxPassword,
    [string]$PfxBase64,
    [string]$TimestampUrl,
    [string]$MakeAppxPath = $env:MAKEAPPX_PATH,
    [string]$SignToolPath = $env:SIGNTOOL_PATH,
    [switch]$SkipBuild
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$outputRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target\msix"))
$stagingDirectory = Join-Path $outputRoot "staging"
$targetTriple = "x86_64-pc-windows-msvc"
$temporaryPfxPath = $null

function Invoke-Native {
    param([Parameter(Mandatory = $true)][string]$FilePath, [Parameter(Mandatory = $true)][string[]]$Arguments)
    $displayArguments = [string[]]$Arguments.Clone()
    if ([IO.Path]::GetFileName($FilePath) -ieq "signtool.exe") {
        for ($index = 0; $index -lt $displayArguments.Length; $index++) {
            if ($index -gt 0 -and $displayArguments[$index - 1] -eq "/p") {
                $displayArguments[$index] = "***"
            }
        }
    }
    Write-Host "[build-msix] $FilePath $($displayArguments -join ' ')"
    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$FilePath failed with exit code $LASTEXITCODE." }
}

function Find-WindowsSdkTool {
    param([Parameter(Mandatory = $true)][string]$ToolName, [string]$ExplicitPath)
    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        $resolved = [IO.Path]::GetFullPath($ExplicitPath)
        if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
            throw "$ToolName was not found at '$resolved'."
        }
        return $resolved
    }
    $sdkRoots = @(
        (Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"),
        (Join-Path $env:ProgramFiles "Windows Kits\10\bin")
    ) | Where-Object { $_ -and (Test-Path -LiteralPath $_ -PathType Container) }
    foreach ($sdkRoot in $sdkRoots) {
        $versions = Get-ChildItem -LiteralPath $sdkRoot -Directory | Sort-Object {
            try { [version]$_.Name } catch { [version]"0.0" }
        } -Descending
        foreach ($versionDirectory in $versions) {
            $candidate = Join-Path $versionDirectory.FullName "x64\$ToolName"
            if (Test-Path -LiteralPath $candidate -PathType Leaf) { return $candidate }
        }
    }
    throw "$ToolName was not found. Install the Windows SDK or provide an explicit tool path."
}

function Reset-OutputDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)
    $fullPath = [IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith($outputRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to reset a directory outside '$outputRoot': $fullPath"
    }
    if (Test-Path -LiteralPath $fullPath) { Remove-Item -LiteralPath $fullPath -Recurse -Force }
    New-Item -ItemType Directory -Path $fullPath -Force | Out-Null
    return $fullPath
}

function Copy-RequiredFile {
    param([Parameter(Mandatory = $true)][string]$Source, [Parameter(Mandatory = $true)][string]$Destination)
    if (-not (Test-Path -LiteralPath $Source -PathType Leaf)) {
        throw "Required build file was not found: $Source"
    }
    New-Item -ItemType Directory -Path (Split-Path -Parent $Destination) -Force | Out-Null
    Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

function Get-WorkspaceVersion {
    $cargoText = Get-Content -LiteralPath (Join-Path $repoRoot "Cargo.toml") -Raw
    $workspacePackage = [regex]::Match($cargoText, '(?ms)^\[workspace\.package\]\s*(.*?)(?=^\[|\z)')
    $versionMatch = [regex]::Match($workspacePackage.Groups[1].Value, '(?m)^\s*version\s*=\s*"([^"]+)"\s*$')
    if (-not $workspacePackage.Success -or -not $versionMatch.Success) {
        throw "Missing [workspace.package].version in root Cargo.toml."
    }
    return $versionMatch.Groups[1].Value
}

function Resolve-Setting {
    param([string]$ParameterValue, [string]$EnvironmentName, [object]$Config, [string]$ConfigProperty, [string]$DefaultValue)
    if (-not [string]::IsNullOrWhiteSpace($ParameterValue)) { return $ParameterValue.Trim() }
    $environmentValue = [Environment]::GetEnvironmentVariable($EnvironmentName)
    if (-not [string]::IsNullOrWhiteSpace($environmentValue)) { return $environmentValue.Trim() }
    if ($null -ne $Config -and $Config.PSObject.Properties.Name -contains $ConfigProperty) {
        $value = [string]$Config.$ConfigProperty
        if (-not [string]::IsNullOrWhiteSpace($value)) { return $value.Trim() }
    }
    return $DefaultValue
}

function Assert-FormalValue {
    param([string]$Value, [string]$FieldName)
    if ([string]::IsNullOrWhiteSpace($Value)) {
        throw "$FieldName is required for a Store package."
    }
    if ($Value -match '(?i)(__PARTNER_CENTER|PLACEHOLDER|CHANGEME|TODO|<[^>]+>)') {
        throw "$FieldName still contains a placeholder. Copy the exact value from Partner Center."
    }
}

function Find-BuildOutput {
    param([Parameter(Mandatory = $true)][string[]]$Candidates)
    foreach ($candidate in $Candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) { return [IO.Path]::GetFullPath($candidate) }
    }
    throw "None of the expected build outputs exist: $($Candidates -join ', ')"
}

if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw "MSIX packaging must run on Windows."
}
if (-not [Environment]::Is64BitOperatingSystem) {
    throw "This workflow requires a 64-bit Windows environment."
}

$config = $null
if (-not [string]::IsNullOrWhiteSpace($ConfigPath)) {
    $resolvedConfigPath = [IO.Path]::GetFullPath($ConfigPath)
    if (-not (Test-Path -LiteralPath $resolvedConfigPath -PathType Leaf)) {
        throw "MSIX config file was not found: $resolvedConfigPath"
    }
    $config = Get-Content -LiteralPath $resolvedConfigPath -Raw | ConvertFrom-Json
}

$workspaceVersion = Get-WorkspaceVersion
$Version = Resolve-Setting $Version "MSIX_VERSION" $config "version" $workspaceVersion
if ($Version -cne $workspaceVersion) {
    throw "Requested MSIX version '$Version' does not match root Cargo.toml version '$workspaceVersion'."
}
$expectedChannel = if ($PackageChannel -eq "Store") { "stable" } else { "test" }
$profileJson = & node (Join-Path $repoRoot "scripts\release-channel.mjs") `
    --version $Version `
    --expected-channel $expectedChannel `
    --distribution microsoft-store `
    --package-channel $PackageChannel `
    --format json
if ($LASTEXITCODE -ne 0) { throw "Version $Version is not compatible with MSIX $PackageChannel." }
$buildProfile = $profileJson | ConvertFrom-Json
$manifestVersion = $buildProfile.msixVersion
$env:UNFOUR_RELEASE_CHANNEL = $buildProfile.releaseChannel
$env:UNFOUR_DISTRIBUTION = $buildProfile.distribution

$IdentityName = Resolve-Setting $IdentityName "MSIX_IDENTITY_NAME" $config "identityName" $null
$Publisher = Resolve-Setting $Publisher "MSIX_PUBLISHER" $config "publisher" $null
$PublisherDisplayName = Resolve-Setting $PublisherDisplayName "MSIX_PUBLISHER_DISPLAY_NAME" $config "publisherDisplayName" $null
$DisplayName = Resolve-Setting $DisplayName "MSIX_DISPLAY_NAME" $config "displayName" "Unfour"
$Description = Resolve-Setting $Description "MSIX_DESCRIPTION" $config "description" "A lightweight developer workbench for API, SSH, Database, and workspace workflows."
$Architecture = Resolve-Setting $Architecture "MSIX_ARCHITECTURE" $config "architecture" "x64"
if ($Architecture -cne "x64") { throw "Only Windows x64 MSIX is implemented." }

if ($PackageChannel -eq "Test") {
    if ([string]::IsNullOrWhiteSpace($IdentityName)) { $IdentityName = "Unfour.LocalTest" }
    if ([string]::IsNullOrWhiteSpace($Publisher)) { $Publisher = "CN=Unfour Local Test" }
    if ([string]::IsNullOrWhiteSpace($PublisherDisplayName)) { $PublisherDisplayName = "Unfour Local Test" }
    if (-not $PSBoundParameters.ContainsKey("DisplayName") -and [string]::IsNullOrWhiteSpace($env:MSIX_DISPLAY_NAME)) {
        $DisplayName = "Unfour (MSIX Test)"
    }
    if ($IdentityName -notmatch '(?i)test' -or $DisplayName -notmatch '(?i)test') {
        throw "PackageChannel Test requires obvious Test markers in IdentityName and DisplayName."
    }
    Write-Warning "Building a local test identity; never submit this artifact to Partner Center."
} else {
    Assert-FormalValue $IdentityName "Identity Name"
    Assert-FormalValue $Publisher "Publisher"
    Assert-FormalValue $PublisherDisplayName "Publisher Display Name"
}
Assert-FormalValue $DisplayName "Display Name"
Assert-FormalValue $Description "Description"

Push-Location $repoRoot
try {
    $MakeAppxPath = Find-WindowsSdkTool "makeappx.exe" $MakeAppxPath
    $resolvedPfxPath = $null
    if ($SignMode -eq "Pfx") {
        $SignToolPath = Find-WindowsSdkTool "signtool.exe" $SignToolPath
        if ([string]::IsNullOrWhiteSpace($PfxPath)) { $PfxPath = $env:MSIX_SIGNING_PFX_PATH }
        if ([string]::IsNullOrWhiteSpace($PfxPassword)) { $PfxPassword = $env:MSIX_SIGNING_CERTIFICATE_PASSWORD }
        if ([string]::IsNullOrWhiteSpace($PfxPassword) -and $PackageChannel -eq "Test") { $PfxPassword = $env:MSIX_TEST_CERTIFICATE_PASSWORD }
        if ([string]::IsNullOrWhiteSpace($PfxBase64)) { $PfxBase64 = $env:MSIX_SIGNING_CERTIFICATE_BASE64 }
        if ([string]::IsNullOrWhiteSpace($TimestampUrl)) { $TimestampUrl = $env:MSIX_TIMESTAMP_URL }
        if (-not [string]::IsNullOrWhiteSpace($PfxBase64)) {
            if (-not [string]::IsNullOrWhiteSpace($PfxPath)) { throw "Provide either PfxPath or PfxBase64, not both." }
            New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
            $temporaryPfxPath = Join-Path $outputRoot ".signing-certificate.pfx"
            try { [IO.File]::WriteAllBytes($temporaryPfxPath, [Convert]::FromBase64String($PfxBase64)) }
            catch { throw "MSIX_SIGNING_CERTIFICATE_BASE64 is not valid base64." }
            $PfxPath = $temporaryPfxPath
        }
        if ([string]::IsNullOrWhiteSpace($PfxPath)) { throw "SignMode=Pfx requires a PFX path or base64 value." }
        $resolvedPfxPath = if ([IO.Path]::IsPathRooted($PfxPath)) { [IO.Path]::GetFullPath($PfxPath) } else { [IO.Path]::GetFullPath((Join-Path $repoRoot $PfxPath)) }
        if (-not (Test-Path -LiteralPath $resolvedPfxPath -PathType Leaf)) { throw "Signing PFX was not found: $resolvedPfxPath" }
        $certificate = New-Object Security.Cryptography.X509Certificates.X509Certificate2($resolvedPfxPath, $PfxPassword)
        try {
            if ($certificate.Subject -cne $Publisher) {
                throw "Signing certificate subject '$($certificate.Subject)' does not exactly match Publisher '$Publisher'."
            }
        } finally { $certificate.Dispose() }
    }

    if (-not $SkipBuild) {
        Invoke-Native "node" @("scripts/sync-version.mjs")
        $previousTarget = $env:UNFOUR_TAURI_TARGET
        try {
            $env:UNFOUR_TAURI_TARGET = $targetTriple
            Invoke-Native "pnpm" @("tauri", "build", "--no-bundle", "--target", $targetTriple)
        } finally { $env:UNFOUR_TAURI_TARGET = $previousTarget }
    }

    $appExecutable = Find-BuildOutput @(
        (Join-Path $repoRoot "target\$targetTriple\release\unfour.exe"),
        (Join-Path $repoRoot "target\release\unfour.exe")
    )
    $mcpExecutable = Find-BuildOutput @(
        (Join-Path $repoRoot "target\tauri-sidecars\$targetTriple\release\unfour-mcp.exe"),
        (Join-Path $repoRoot "target\$targetTriple\release\unfour-mcp.exe"),
        (Join-Path $repoRoot "target\release\unfour-mcp.exe")
    )

    $buildMetadataPath = Join-Path $outputRoot "unfour-build-metadata.json"
    New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
    Remove-Item -LiteralPath $buildMetadataPath -Force -ErrorAction SilentlyContinue
    $metadataProcess = Start-Process -FilePath $appExecutable -ArgumentList @("--write-build-metadata", "`"$buildMetadataPath`"") -WindowStyle Hidden -Wait -PassThru
    if ($metadataProcess.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $buildMetadataPath)) {
        throw "Application build metadata command failed."
    }
    $compiledMetadata = Get-Content -LiteralPath $buildMetadataPath -Raw | ConvertFrom-Json
    $expectedMetadata = [ordered]@{
        version = $buildProfile.version
        distribution = "microsoft-store"
        channel = $buildProfile.releaseChannel
        profile = $buildProfile.kind
        accountApiUrl = $buildProfile.accountApiUrl
        accountWebUrl = $buildProfile.accountWebUrl
        updaterEnabled = $false
        updaterEndpoint = $null
        defaultStorageProfile = $buildProfile.defaultStorageProfile
    }
    foreach ($entry in $expectedMetadata.GetEnumerator()) {
        if ([string]$compiledMetadata.($entry.Key) -cne [string]$entry.Value) {
            throw "Compiled metadata $($entry.Key) mismatch. Refusing to wrap a stale or cross-channel binary."
        }
    }
    if ([string]::IsNullOrWhiteSpace([string]$compiledMetadata.commit)) {
        throw "MSIX packaging requires the unified repository commit."
    }

    $stagingDirectory = Reset-OutputDirectory $stagingDirectory
    Copy-RequiredFile $appExecutable (Join-Path $stagingDirectory "unfour.exe")
    Copy-RequiredFile $mcpExecutable (Join-Path $stagingDirectory "unfour-mcp.exe")
    Copy-RequiredFile $buildMetadataPath (Join-Path $stagingDirectory "unfour-build-metadata.json")
    $iconRoot = Join-Path $repoRoot "apps\desktop\src-tauri\icons"
    foreach ($iconName in @("StoreLogo.png", "Square44x44Logo.png", "Square71x71Logo.png", "Square150x150Logo.png", "Square310x310Logo.png")) {
        Copy-RequiredFile (Join-Path $iconRoot $iconName) (Join-Path $stagingDirectory "Assets\$iconName")
    }

    $manifestText = Get-Content -LiteralPath (Join-Path $PSScriptRoot "AppxManifest.template.xml") -Raw
    $replacements = [ordered]@{
        "{{IDENTITY_NAME}}" = [Security.SecurityElement]::Escape($IdentityName)
        "{{PUBLISHER}}" = [Security.SecurityElement]::Escape($Publisher)
        "{{PUBLISHER_DISPLAY_NAME}}" = [Security.SecurityElement]::Escape($PublisherDisplayName)
        "{{DISPLAY_NAME}}" = [Security.SecurityElement]::Escape($DisplayName)
        "{{DESCRIPTION}}" = [Security.SecurityElement]::Escape($Description)
        "{{VERSION}}" = $manifestVersion
        "{{ARCHITECTURE}}" = $Architecture
    }
    foreach ($entry in $replacements.GetEnumerator()) { $manifestText = $manifestText.Replace($entry.Key, $entry.Value) }
    $manifestPath = Join-Path $stagingDirectory "AppxManifest.xml"
    [IO.File]::WriteAllText($manifestPath, $manifestText, (New-Object Text.UTF8Encoding($false)))

    $validator = Join-Path $PSScriptRoot "validate-msix.ps1"
    & pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -StagingDirectory $stagingDirectory -ExpectedIdentityName $IdentityName -ExpectedPublisher $Publisher -ExpectedPublisherDisplayName $PublisherDisplayName -ExpectedVersion $manifestVersion -ExpectedArchitecture $Architecture -ExpectedDisplayName $DisplayName -ExpectedPackageChannel $PackageChannel
    if ($LASTEXITCODE -ne 0) { throw "MSIX staging validation failed." }

    $signatureLabel = if ($SignMode -eq "Pfx") { "SIGNED" } else { "UNSIGNED" }
    $outputPath = Join-Path $outputRoot "Unfour_$($manifestVersion)_$($Architecture)_$($buildProfile.artifactLabel)-$signatureLabel.msix"
    Invoke-Native $MakeAppxPath @("pack", "/v", "/o", "/h", "SHA256", "/d", $stagingDirectory, "/p", $outputPath)
    if ($SignMode -eq "Pfx") {
        $signArguments = @("sign", "/fd", "SHA256", "/a", "/f", $resolvedPfxPath)
        if (-not [string]::IsNullOrEmpty($PfxPassword)) { $signArguments += @("/p", $PfxPassword) }
        if (-not [string]::IsNullOrWhiteSpace($TimestampUrl)) { $signArguments += @("/tr", $TimestampUrl, "/td", "SHA256") }
        $signArguments += $outputPath
        Invoke-Native $SignToolPath $signArguments
        Invoke-Native $SignToolPath @("verify", "/pa", "/v", $outputPath)
    } else {
        Write-Warning "Generated an unsigned MSIX. Partner Center can sign a correctly identified Store submission."
    }

    $hash = (Get-FileHash -LiteralPath $outputPath -Algorithm SHA256).Hash.ToUpperInvariant()
    $checksumPath = "$outputPath.sha256"
    [IO.File]::WriteAllText($checksumPath, "$hash *$([IO.Path]::GetFileName($outputPath))`r`n", (New-Object Text.UTF8Encoding($false)))
    & pwsh -NoProfile -ExecutionPolicy Bypass -File $validator -MsixPath $outputPath -MakeAppxPath $MakeAppxPath -ExpectedIdentityName $IdentityName -ExpectedPublisher $Publisher -ExpectedPublisherDisplayName $PublisherDisplayName -ExpectedVersion $manifestVersion -ExpectedArchitecture $Architecture -ExpectedDisplayName $DisplayName -ExpectedPackageChannel $PackageChannel
    if ($LASTEXITCODE -ne 0) { throw "Packed MSIX validation failed." }

    Write-Host "[build-msix] MSIX: $outputPath"
    Write-Host "[build-msix] SHA-256: $hash"
    Write-Host "[build-msix] Updates: managed by Microsoft Store"
    Write-Host "[build-msix] Protocol: unfour://"
    Write-Host "[build-msix] MCP command: unfour-mcp.exe"
} finally {
    if ($null -ne $temporaryPfxPath -and (Test-Path -LiteralPath $temporaryPfxPath)) {
        Remove-Item -LiteralPath $temporaryPfxPath -Force
    }
    Pop-Location
}
