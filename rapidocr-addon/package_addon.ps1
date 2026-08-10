param(
    [Parameter(Mandatory = $true)]
    [string]$PythonExe,
    [string]$Version = "3.9.2-1"
)

$ErrorActionPreference = "Stop"
$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$BuildRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptRoot "build"))
$DistRoot = Join-Path $BuildRoot "dist\rapidocr_worker"
$PackageRoot = Join-Path $BuildRoot "package"
$ReleaseRoot = Join-Path $ScriptRoot "release"
$OutputZip = Join-Path $ReleaseRoot ("TempleFix_RapidOCR_Addon_" + $Version + ".zip")

function Assert-BuildPath([string]$Path) {
    $full = [System.IO.Path]::GetFullPath($Path)
    if (-not $full.StartsWith($BuildRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase) -and $full -ne $BuildRoot) {
        throw "Refusing to modify a path outside the add-on build directory: $full"
    }
}

if (-not (Test-Path -LiteralPath (Join-Path $DistRoot "rapidocr_worker.exe"))) {
    throw "PyInstaller output is missing. Build rapidocr_worker first."
}

Assert-BuildPath $PackageRoot
if (Test-Path -LiteralPath $PackageRoot) {
    Remove-Item -LiteralPath $PackageRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $PackageRoot | Out-Null
Copy-Item -Path (Join-Path $DistRoot "*") -Destination $PackageRoot -Recurse -Force

# Video decoding is not used by screenshot OCR. Removing this optional OpenCV
# codec saves about 27 MB while image decoding and OCR remain functional.
$ffmpeg = Get-ChildItem -LiteralPath (Join-Path $PackageRoot "_internal\cv2") -Filter "opencv_videoio_ffmpeg*.dll" -ErrorAction SilentlyContinue
foreach ($file in $ffmpeg) {
    Assert-BuildPath $file.FullName
    Remove-Item -LiteralPath $file.FullName -Force
}

Copy-Item -LiteralPath (Join-Path $ScriptRoot "THIRD_PARTY_NOTICES.txt") -Destination $PackageRoot -Force
$LicenseRoot = Join-Path $PackageRoot "licenses"
New-Item -ItemType Directory -Path $LicenseRoot | Out-Null

# RapidOCR and PaddleOCR are both Apache-2.0 projects. Their wheels/model
# files do not consistently ship a standalone license file, so include the
# project's full Apache-2.0 text explicitly in the component.
$projectLicense = Join-Path (Split-Path -Parent $ScriptRoot) "LICENSE"
if (-not (Test-Path -LiteralPath $projectLicense)) {
    throw "The project Apache-2.0 license file is missing."
}
Copy-Item -LiteralPath $projectLicense -Destination (Join-Path $LicenseRoot "RapidOCR_and_PaddleOCR_Apache-2.0.txt") -Force

$sitePackages = (& $PythonExe -c "import site; print(site.getsitepackages()[0])").Trim()
$licensePatterns = @("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*", "ThirdPartyNotices*")
$licenseFiles = foreach ($pattern in $licensePatterns) {
    Get-ChildItem -LiteralPath $sitePackages -Recurse -File -Filter $pattern -ErrorAction SilentlyContinue
}
$licenseFiles = $licenseFiles | Sort-Object FullName -Unique
foreach ($file in $licenseFiles) {
    $relative = $file.FullName.Substring($sitePackages.Length).TrimStart("\")
    $safeName = $relative -replace '[\\/:*?"<>|]', "__"
    Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $LicenseRoot $safeName) -Force
}

$pythonLicense = (& $PythonExe -c "import sys, pathlib; print(pathlib.Path(sys.base_prefix) / 'LICENSE.txt')").Trim()
if (Test-Path -LiteralPath $pythonLicense) {
    Copy-Item -LiteralPath $pythonLicense -Destination (Join-Path $LicenseRoot "Python_LICENSE.txt") -Force
}

$languages = @(
    "Chinese (Simplified)", "Chinese (Traditional)", "English", "Japanese",
    "Afrikaans", "Azerbaijani", "Bosnian", "Catalan", "Czech",
    "Welsh", "Danish", "German", "Estonian", "Basque", "Finnish",
    "French", "Irish", "Galician", "Croatian", "Hungarian",
    "Indonesian", "Icelandic", "Italian", "Kurdish", "Latin",
    "Luxembourgish", "Lithuanian", "Latvian", "Maori", "Malay",
    "Maltese", "Dutch", "Norwegian", "Occitan", "Polish",
    "Portuguese", "Quechua", "Romansh", "Romanian", "Serbian (Latin)",
    "Slovak", "Slovenian", "Albanian", "Spanish", "Swedish",
    "Swahili", "Filipino", "Turkish", "Uzbek", "Vietnamese"
)

$criticalFiles = @(
    "rapidocr_worker.exe",
    "_internal/rapidocr/models/PP-OCRv6_det_small.onnx",
    "_internal/rapidocr/models/PP-OCRv6_rec_small.onnx",
    "_internal/rapidocr/models/ch_ppocr_mobile_v2.0_cls_mobile.onnx"
)
$checksums = [ordered]@{}
foreach ($relative in $criticalFiles) {
    $path = Join-Path $PackageRoot ($relative -replace "/", "\")
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Critical component file is missing: $relative"
    }
    $checksums[$relative] = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
}

$installedBytes = (Get-ChildItem -LiteralPath $PackageRoot -Recurse -File | Measure-Object -Property Length -Sum).Sum
$manifest = [ordered]@{
    schema = 1
    name = "TempleFix RapidOCR"
    version = $Version
    engine = "RapidOCR 3.9.2 / PP-OCRv6 small / ONNX Runtime CPU"
    worker = "rapidocr_worker.exe"
    installed_bytes = [uint64]$installedBytes
    supported_languages = $languages
    checksums = $checksums
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $PackageRoot "manifest.json") -Encoding UTF8

New-Item -ItemType Directory -Path $ReleaseRoot -Force | Out-Null
if (Test-Path -LiteralPath $OutputZip) {
    Remove-Item -LiteralPath $OutputZip -Force
}
Compress-Archive -Path (Join-Path $PackageRoot "*") -DestinationPath $OutputZip -CompressionLevel Optimal

$zip = Get-Item -LiteralPath $OutputZip
$hash = Get-FileHash -LiteralPath $OutputZip -Algorithm SHA256
[pscustomobject]@{
    Package = $zip.FullName
    DownloadMB = [math]::Round($zip.Length / 1MB, 1)
    InstalledMB = [math]::Round($installedBytes / 1MB, 1)
    SHA256 = $hash.Hash
}
