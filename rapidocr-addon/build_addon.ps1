param(
    [string]$PythonExe = "python",
    [string]$Version = "3.9.2-1"
)

$ErrorActionPreference = "Stop"
$ScriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$BuildRoot = [System.IO.Path]::GetFullPath((Join-Path $ScriptRoot "build"))
$VenvRoot = Join-Path $BuildRoot "venv"
$VenvPython = Join-Path $VenvRoot "Scripts\python.exe"

New-Item -ItemType Directory -Path $BuildRoot -Force | Out-Null
if (-not (Test-Path -LiteralPath $VenvPython)) {
    & $PythonExe -m venv $VenvRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to create the isolated Python environment."
    }
}

& $VenvPython -m pip install --disable-pip-version-check --no-deps -r (Join-Path $ScriptRoot "requirements-lock.txt")
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install the pinned component dependencies."
}

& $VenvPython -m PyInstaller `
    --noconfirm `
    --clean `
    --onedir `
    --name rapidocr_worker `
    --distpath (Join-Path $BuildRoot "dist") `
    --workpath (Join-Path $BuildRoot "pyinstaller") `
    --specpath $BuildRoot `
    --collect-all rapidocr `
    --hidden-import onnxruntime `
    --exclude-module torch `
    --exclude-module paddle `
    --exclude-module openvino `
    --exclude-module tensorrt `
    --exclude-module mnn `
    (Join-Path $ScriptRoot "worker.py")
if ($LASTEXITCODE -ne 0) {
    throw "Failed to build the RapidOCR worker."
}

& (Join-Path $ScriptRoot "package_addon.ps1") -PythonExe $VenvPython -Version $Version
if ($LASTEXITCODE -ne 0) {
    throw "Failed to package the RapidOCR component."
}
