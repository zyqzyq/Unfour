[CmdletBinding()]
param(
    [string]$MsixPath,
    [string]$StagingDirectory,
    [string]$MakeAppxPath = $env:MAKEAPPX_PATH,
    [string]$ExpectedIdentityName,
    [string]$ExpectedPublisher,
    [string]$ExpectedPublisherDisplayName,
    [string]$ExpectedVersion,
    [string]$ExpectedDisplayName,
    [ValidateSet("", "Store", "Test")]
    [string]$ExpectedPackageChannel = "",
    [ValidateSet("x64")]
    [string]$ExpectedArchitecture = "x64"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\.."))
$outputRoot = [System.IO.Path]::GetFullPath((Join-Path $repoRoot "target\msix"))
$temporaryDirectory = $null

function Find-WindowsSdkTool {
    param([Parameter(Mandatory = $true)][string]$ToolName, [string]$ExplicitPath)
    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        $resolved = [System.IO.Path]::GetFullPath($ExplicitPath)
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
    throw "$ToolName was not found. Install the Windows SDK and/or set MAKEAPPX_PATH."
}

function Reset-TemporaryDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)
    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $allowedPrefix = $outputRoot.TrimEnd('\') + '\'
    if (-not $fullPath.StartsWith($allowedPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Refusing to reset temporary directory outside '$outputRoot': $fullPath"
    }
    if (Test-Path -LiteralPath $fullPath) {
        Remove-Item -LiteralPath $fullPath -Recurse -Force
    }
    New-Item -ItemType Directory -Path $fullPath -Force | Out-Null
    return $fullPath
}

function Assert-File {
    param([Parameter(Mandatory = $true)][string]$Root, [Parameter(Mandatory = $true)][string]$RelativePath)
    $path = Join-Path $Root $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required MSIX payload file is missing: $RelativePath"
    }
    if ((Get-Item -LiteralPath $path).Length -le 0) {
        throw "Required MSIX payload file is empty: $RelativePath"
    }
}

function Assert-ExpectedValue {
    param([string]$Actual, [string]$Expected, [string]$FieldName)
    if (-not [string]::IsNullOrWhiteSpace($Expected) -and $Actual -cne $Expected) {
        throw "$FieldName mismatch. Expected '$Expected', found '$Actual'."
    }
}

try {
    if ([string]::IsNullOrWhiteSpace($StagingDirectory)) {
        if ([string]::IsNullOrWhiteSpace($MsixPath)) {
            $latestPackage = Get-ChildItem -LiteralPath $outputRoot -Filter "*.msix" -File -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1
            if ($null -eq $latestPackage) {
                throw "No MSIX path was provided and no package exists under '$outputRoot'."
            }
            $MsixPath = $latestPackage.FullName
        }
        $MsixPath = [System.IO.Path]::GetFullPath($MsixPath)
        if (-not (Test-Path -LiteralPath $MsixPath -PathType Leaf)) {
            throw "MSIX package was not found: $MsixPath"
        }
        if ((Get-Item -LiteralPath $MsixPath).Length -le 0) {
            throw "MSIX package is empty: $MsixPath"
        }
        $MakeAppxPath = Find-WindowsSdkTool -ToolName "makeappx.exe" -ExplicitPath $MakeAppxPath
        $temporaryDirectory = Reset-TemporaryDirectory -Path (Join-Path $outputRoot "validate-unpacked")
        & $MakeAppxPath unpack /p $MsixPath /d $temporaryDirectory /o | Out-Host
        if ($LASTEXITCODE -ne 0) { throw "MakeAppx unpack failed with exit code $LASTEXITCODE." }
        $StagingDirectory = $temporaryDirectory
    } else {
        $StagingDirectory = [System.IO.Path]::GetFullPath($StagingDirectory)
        if (-not (Test-Path -LiteralPath $StagingDirectory -PathType Container)) {
            throw "MSIX staging directory was not found: $StagingDirectory"
        }
    }

    $requiredFiles = @(
        "AppxManifest.xml", "unfour.exe", "unfour-mcp.exe", "unfour-build-metadata.json",
        "Assets\StoreLogo.png", "Assets\Square44x44Logo.png", "Assets\Square71x71Logo.png",
        "Assets\Square150x150Logo.png", "Assets\Square310x310Logo.png"
    )
    foreach ($requiredFile in $requiredFiles) {
        Assert-File -Root $StagingDirectory -RelativePath $requiredFile
    }

    $manifestPath = Join-Path $StagingDirectory "AppxManifest.xml"
    $manifestText = Get-Content -LiteralPath $manifestPath -Raw
    if ($manifestText -match "\{\{[^}]+\}\}" -or $manifestText -match "__(?:PARTNER_CENTER|PLACEHOLDER)") {
        throw "AppxManifest.xml contains unresolved placeholder values."
    }
    [xml]$manifest = $manifestText
    $namespaces = New-Object System.Xml.XmlNamespaceManager($manifest.NameTable)
    $namespaces.AddNamespace("p", "http://schemas.microsoft.com/appx/manifest/foundation/windows10")
    $namespaces.AddNamespace("uap3", "http://schemas.microsoft.com/appx/manifest/uap/windows10/3")
    $namespaces.AddNamespace("desktop", "http://schemas.microsoft.com/appx/manifest/desktop/windows10")
    $namespaces.AddNamespace("rescap", "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities")

    $identity = $manifest.SelectSingleNode("/p:Package/p:Identity", $namespaces)
    if ($null -eq $identity) { throw "AppxManifest.xml is missing Package/Identity." }
    $identityName = $identity.GetAttribute("Name")
    $publisher = $identity.GetAttribute("Publisher")
    $version = $identity.GetAttribute("Version")
    $architecture = $identity.GetAttribute("ProcessorArchitecture")
    if ([string]::IsNullOrWhiteSpace($identityName) -or [string]::IsNullOrWhiteSpace($publisher)) {
        throw "MSIX Identity Name and Publisher must not be empty."
    }
    if ($version -notmatch "^(\d+)\.(\d+)\.(\d+)\.(\d+)$") {
        throw "MSIX Identity Version must be a numeric four-part version, found '$version'."
    }
    foreach ($segment in $Matches[1..4]) {
        if ([int64]$segment -gt 65535) { throw "MSIX Identity Version segment exceeds 65535: '$version'." }
    }
    if ($architecture -cne "x64") {
        throw "This workflow only supports ProcessorArchitecture='x64', found '$architecture'."
    }
    Assert-ExpectedValue $identityName $ExpectedIdentityName "Identity Name"
    Assert-ExpectedValue $publisher $ExpectedPublisher "Publisher"
    Assert-ExpectedValue $version $ExpectedVersion "Version"
    Assert-ExpectedValue $architecture $ExpectedArchitecture "Architecture"

    $displayNameNode = $manifest.SelectSingleNode("/p:Package/p:Properties/p:DisplayName", $namespaces)
    $publisherDisplayNameNode = $manifest.SelectSingleNode("/p:Package/p:Properties/p:PublisherDisplayName", $namespaces)
    if ($null -eq $displayNameNode -or $null -eq $publisherDisplayNameNode) {
        throw "AppxManifest.xml is missing display-name properties."
    }
    $displayName = $displayNameNode.InnerText
    $publisherDisplayName = $publisherDisplayNameNode.InnerText
    Assert-ExpectedValue $displayName $ExpectedDisplayName "Display Name"
    Assert-ExpectedValue $publisherDisplayName $ExpectedPublisherDisplayName "Publisher Display Name"

    $metadata = Get-Content -LiteralPath (Join-Path $StagingDirectory "unfour-build-metadata.json") -Raw | ConvertFrom-Json
    if ([string]::IsNullOrWhiteSpace($ExpectedPackageChannel)) {
        $ExpectedPackageChannel = if ([string]$metadata.channel -ceq "stable") { "Store" } else { "Test" }
    }
    $profileJson = & node (Join-Path $repoRoot "scripts\release-channel.mjs") `
        --version ([string]$metadata.version) `
        --expected-channel ([string]$metadata.channel) `
        --distribution microsoft-store `
        --package-channel $ExpectedPackageChannel `
        --format json
    if ($LASTEXITCODE -ne 0) {
        throw "Packaged application metadata is incompatible with PackageChannel $ExpectedPackageChannel."
    }
    $profile = $profileJson | ConvertFrom-Json
    $expectations = [ordered]@{
        version = $profile.version
        distribution = "microsoft-store"
        channel = $profile.releaseChannel
        profile = $profile.kind
        accountApiUrl = $profile.accountApiUrl
        accountWebUrl = $profile.accountWebUrl
        updaterEnabled = $false
        updaterEndpoint = $null
        defaultStorageProfile = $profile.defaultStorageProfile
    }
    foreach ($entry in $expectations.GetEnumerator()) {
        if ([string]$metadata.($entry.Key) -cne [string]$entry.Value) {
            throw "Packaged application metadata $($entry.Key) mismatch. Expected '$($entry.Value)', found '$($metadata.($entry.Key))'."
        }
    }
    if ([string]::IsNullOrWhiteSpace([string]$metadata.commit)) {
        throw "Packaged application metadata must contain the unified repository commit."
    }
    if ($ExpectedPackageChannel -eq "Store") {
        if ([string]$metadata.channel -cne "stable" -or [string]$metadata.profile -cne "stable") {
            throw "A Store package may contain only a plain X.Y.Z Stable application build."
        }
        if ([bool]$metadata.updaterEnabled -or $null -ne $metadata.updaterEndpoint) {
            throw "A Store package must disable the standard updater and use a null updater endpoint."
        }
    } elseif ($identityName -notmatch '(?i)test' -or $displayName -notmatch '(?i)test') {
        throw "A Test package must have obvious Test identity and display-name markers."
    }

    $application = $manifest.SelectSingleNode("/p:Package/p:Applications/p:Application", $namespaces)
    if ($null -eq $application -or $application.GetAttribute("Executable") -cne "unfour.exe") {
        throw "The manifest must launch unfour.exe as the primary application."
    }
    if ($application.GetAttribute("EntryPoint") -cne "Windows.FullTrustApplication") {
        throw "The primary application must use Windows.FullTrustApplication."
    }
    if ($null -eq $manifest.SelectSingleNode("/p:Package/p:Capabilities/rescap:Capability[@Name='runFullTrust']", $namespaces)) {
        throw "The manifest must declare the runFullTrust restricted capability."
    }

    $aliasExtension = $manifest.SelectSingleNode("/p:Package/p:Applications/p:Application/p:Extensions/uap3:Extension[@Category='windows.appExecutionAlias']", $namespaces)
    if ($null -eq $aliasExtension -or $aliasExtension.GetAttribute("Executable") -cne "unfour-mcp.exe") {
        throw "The manifest must map windows.appExecutionAlias to unfour-mcp.exe."
    }
    $alias = $aliasExtension.SelectSingleNode("uap3:AppExecutionAlias/desktop:ExecutionAlias", $namespaces)
    if ($null -eq $alias -or $alias.GetAttribute("Alias") -cne "unfour-mcp.exe") {
        throw "The stable MCP execution alias 'unfour-mcp.exe' is missing."
    }

    $protocolExtension = $manifest.SelectSingleNode("/p:Package/p:Applications/p:Application/p:Extensions/uap3:Extension[@Category='windows.protocol']", $namespaces)
    if ($null -eq $protocolExtension -or $protocolExtension.GetAttribute("Executable") -cne "unfour.exe") {
        throw "The manifest must map windows.protocol to unfour.exe."
    }
    if ($protocolExtension.GetAttribute("EntryPoint") -cne "Windows.FullTrustApplication") {
        throw "The unfour protocol must use Windows.FullTrustApplication."
    }
    $protocol = $protocolExtension.SelectSingleNode("uap3:Protocol", $namespaces)
    if ($null -eq $protocol -or $protocol.GetAttribute("Name") -cne "unfour") {
        throw "The manifest must register the exact unfour:// protocol scheme."
    }
    if ($protocol.GetAttribute("Parameters") -notmatch '%1') {
        throw "The unfour:// protocol must forward the activation URI to unfour.exe."
    }

    if (-not [string]::IsNullOrWhiteSpace($MsixPath)) {
        $signature = Get-AuthenticodeSignature -FilePath $MsixPath
        if ($signature.Status -notin @("Valid", "NotSigned")) {
            throw "MSIX signature status is '$($signature.Status)': $($signature.StatusMessage)"
        }
        $checksumPath = "$MsixPath.sha256"
        if (Test-Path -LiteralPath $checksumPath -PathType Leaf) {
            $expectedHash = ((Get-Content -LiteralPath $checksumPath -Raw).Trim() -split "\s+")[0].ToUpperInvariant()
            $actualHash = (Get-FileHash -LiteralPath $MsixPath -Algorithm SHA256).Hash.ToUpperInvariant()
            if ($expectedHash -cne $actualHash) { throw "SHA-256 mismatch for '$MsixPath'." }
        }
        Write-Host "[validate-msix] Signature status: $($signature.Status)"
    }

    Write-Host "[validate-msix] PASS"
    Write-Host "[validate-msix] Identity: $identityName"
    Write-Host "[validate-msix] Version: $version"
    Write-Host "[validate-msix] Package channel: $ExpectedPackageChannel"
    Write-Host "[validate-msix] Distribution: $($metadata.distribution)"
    Write-Host "[validate-msix] Updater enabled: $($metadata.updaterEnabled)"
    Write-Host "[validate-msix] Protocol: unfour://"
    Write-Host "[validate-msix] MCP alias: unfour-mcp.exe"
} finally {
    if ($null -ne $temporaryDirectory -and (Test-Path -LiteralPath $temporaryDirectory)) {
        Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force
    }
}
