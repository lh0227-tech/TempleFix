$ErrorActionPreference = "Stop"

$addonRoot = $PSScriptRoot
$projectRoot = Split-Path -Parent $addonRoot
$metadataPath = Join-Path $addonRoot "modelscope\release.json"
$metadata = Get-Content -LiteralPath $metadataPath -Raw -Encoding UTF8 | ConvertFrom-Json
$packagePath = Join-Path $addonRoot ("release\" + $metadata.package_name)
$uploadPath = Join-Path $addonRoot "modelscope-upload"

if (-not (Test-Path -LiteralPath $packagePath -PathType Leaf)) {
    throw "找不到正式组件包：$packagePath"
}

$package = Get-Item -LiteralPath $packagePath
if ($package.Length -ne [int64]$metadata.package_bytes) {
    throw "组件包体积不符：应为 $($metadata.package_bytes)，实际为 $($package.Length)"
}

$actualHash = (Get-FileHash -LiteralPath $packagePath -Algorithm SHA256).Hash
if ($actualHash -ne $metadata.sha256) {
    throw "组件包 SHA-256 不符：$actualHash"
}

if (Test-Path -LiteralPath $uploadPath) {
    $resolvedUpload = (Resolve-Path -LiteralPath $uploadPath).Path
    $resolvedAddon = (Resolve-Path -LiteralPath $addonRoot).Path
    if (-not $resolvedUpload.StartsWith($resolvedAddon + "\", [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "拒绝清理组件目录以外的路径：$resolvedUpload"
    }
    Remove-Item -LiteralPath $resolvedUpload -Recurse -Force
}

New-Item -ItemType Directory -Path $uploadPath | Out-Null
Copy-Item -LiteralPath (Join-Path $addonRoot "modelscope\README.md") -Destination $uploadPath
Copy-Item -LiteralPath $metadataPath -Destination $uploadPath
Copy-Item -LiteralPath $packagePath -Destination $uploadPath

Write-Host "ModelScope 上传目录已准备：$uploadPath"
Write-Host "组件：$($metadata.package_name)"
Write-Host "SHA-256：$actualHash"
