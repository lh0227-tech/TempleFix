param(
    [Parameter(Mandatory = $true)][string]$LatestJsonPath,
    [Parameter(Mandatory = $true)][string]$GiteeBundleUrl,
    [Parameter(Mandatory = $true)][string]$OutputPath
)

$ErrorActionPreference = "Stop"

$bundleUri = [Uri]$GiteeBundleUrl
if ($bundleUri.Scheme -ne "https" -or
    $bundleUri.Host -notin @("gitee.com", "www.gitee.com") -or
    -not [string]::IsNullOrEmpty($bundleUri.Query) -or
    -not [string]::IsNullOrEmpty($bundleUri.Fragment) -or
    -not [string]::IsNullOrEmpty($bundleUri.UserInfo)) {
    throw "GiteeBundleUrl must be an exact public HTTPS download URL on gitee.com without credentials, query parameters, or fragments."
}

$metadata = Get-Content -LiteralPath $LatestJsonPath -Raw | ConvertFrom-Json
if ([string]::IsNullOrWhiteSpace([string]$metadata.version)) {
    throw "The updater metadata does not contain a version."
}
if ([string]$metadata.version -notmatch '^v?(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$') {
    throw "The updater metadata version is invalid."
}

$windows = $metadata.platforms.'windows-x86_64'
if ($null -eq $windows -or [string]::IsNullOrWhiteSpace([string]$windows.signature)) {
    throw "The updater metadata does not contain a signed windows-x86_64 package."
}

$windows.url = $bundleUri.AbsoluteUri
$metadata | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $OutputPath -Encoding utf8

Write-Host "Gitee updater metadata prepared at $OutputPath. No files were uploaded."
