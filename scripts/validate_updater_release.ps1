$ErrorActionPreference = "Stop"

function Assert-ConfiguredValue {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [AllowNull()][AllowEmptyString()][string]$Value
    )

    if ([string]::IsNullOrWhiteSpace($Value) -or $Value.Contains("PLACEHOLDER")) {
        throw "$Name is not configured. Release creation is intentionally blocked."
    }
}

Assert-ConfiguredValue "TAURI_SIGNING_PRIVATE_KEY" $env:TAURI_SIGNING_PRIVATE_KEY

$projectRoot = Split-Path -Parent $PSScriptRoot
$trackedPublicKey = (Get-Content -LiteralPath (Join-Path $projectRoot "src-tauri\updater-public.key") -Raw).Trim()
Assert-ConfiguredValue "src-tauri/updater-public.key" $trackedPublicKey

try {
    $publicKeyFile = [Text.Encoding]::UTF8.GetString(
        [Convert]::FromBase64String($trackedPublicKey)
    )
    $publicKeyLines = @($publicKeyFile -split "`r?`n" | Where-Object { -not [string]::IsNullOrWhiteSpace($_) })
    if ($publicKeyLines.Count -lt 2) {
        throw "missing minisign public key line"
    }
    $minisignKey = [Convert]::FromBase64String($publicKeyLines[1].Trim())
    if ($minisignKey.Length -ne 42 -or
        $minisignKey[0] -ne 0x45 -or
        $minisignKey[1] -notin @(0x44, 0x64)) {
        throw "invalid minisign updater public key"
    }
}
catch {
    throw "src-tauri/updater-public.key is not a valid Tauri-encoded Minisign public key."
}

if (-not [string]::IsNullOrWhiteSpace($env:TEMPLEFIX_GITEE_UPDATE_ENDPOINT)) {
    $giteeEndpoint = [Uri]$env:TEMPLEFIX_GITEE_UPDATE_ENDPOINT
    if ($giteeEndpoint.Scheme -ne "https" -or
        $giteeEndpoint.Host -notin @("gitee.com", "www.gitee.com") -or
        -not [string]::IsNullOrEmpty($giteeEndpoint.Query) -or
        -not [string]::IsNullOrEmpty($giteeEndpoint.Fragment) -or
        -not [string]::IsNullOrEmpty($giteeEndpoint.UserInfo)) {
        throw "TEMPLEFIX_GITEE_UPDATE_ENDPOINT must be a public HTTPS URL on gitee.com without credentials, query parameters, or fragments."
    }
}
else {
    Write-Host "Gitee mirror is not configured; preparing a GitHub-only release."
}

$cargoToml = Get-Content -LiteralPath (Join-Path $projectRoot "src-tauri\Cargo.toml") -Raw
$tauriConfig = Get-Content -LiteralPath (Join-Path $projectRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$updaterConfig = Get-Content -LiteralPath (Join-Path $projectRoot "src-tauri\tauri.updater.conf.json") -Raw | ConvertFrom-Json
$bundledPublicKey = [string]$updaterConfig.plugins.updater.pubkey
if ([string]::IsNullOrWhiteSpace($bundledPublicKey) -or
    $bundledPublicKey.Trim() -ne $trackedPublicKey) {
    throw "The bundled updater public key and src-tauri/updater-public.key do not match."
}
$cargoVersionMatch = [regex]::Match($cargoToml, '(?m)^version\s*=\s*"(?<version>[^"]+)"')
if (-not $cargoVersionMatch.Success) {
    throw "Could not read the Cargo package version."
}
if ($cargoVersionMatch.Groups["version"].Value -ne [string]$tauriConfig.version) {
    throw "Cargo.toml and tauri.conf.json versions do not match."
}

Write-Host "Signed updater release configuration is complete for version $($tauriConfig.version)."
