$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $projectRoot "src-tauri\Cargo.toml"

function Assert-LastExitCode {
    param([string]$Step)
    if ($LASTEXITCODE -ne 0) {
        throw "$Step failed with exit code $LASTEXITCODE"
    }
}

Push-Location $projectRoot
try {
    cargo fmt --manifest-path $manifest -- --check
    Assert-LastExitCode "Rust format check"

    cargo clippy --manifest-path $manifest --all-targets --locked -- -D warnings
    Assert-LastExitCode "Rust static check"

    cargo test --manifest-path $manifest --locked
    Assert-LastExitCode "Rust tests"

    node scripts\test_frontend_contract.js
    Assert-LastExitCode "Frontend contract check"

    node scripts\test_result_overlay.js
    Assert-LastExitCode "Result overlay rendering check"

    node scripts\test_update_contract.js
    Assert-LastExitCode "Application update contract check"

    $updateTestDirectory = Join-Path $projectRoot "src-tauri\target\test-output"
    New-Item -ItemType Directory -Force -Path $updateTestDirectory | Out-Null
    $giteeMetadataPath = Join-Path $updateTestDirectory "latest.gitee.json"
    & (Join-Path $projectRoot "scripts\prepare_gitee_update.ps1") `
        -LatestJsonPath (Join-Path $projectRoot "scripts\testdata\latest.updater.json") `
        -GiteeBundleUrl "https://gitee.com/example/templefix/releases/download/v1.2.3/TempleFix.msi.zip" `
        -OutputPath $giteeMetadataPath
    $giteeMetadata = Get-Content -LiteralPath $giteeMetadataPath -Raw | ConvertFrom-Json
    if ($giteeMetadata.version -ne "1.2.3-beta.1" -or
        $giteeMetadata.platforms.'windows-x86_64'.url -notlike "https://gitee.com/*" -or
        [string]::IsNullOrWhiteSpace($giteeMetadata.platforms.'windows-x86_64'.signature)) {
        throw "Gitee updater metadata check failed"
    }

    Get-ChildItem -LiteralPath (Join-Path $projectRoot "src\js") -Filter "*.js" |
        ForEach-Object {
            node --check $_.FullName
            Assert-LastExitCode "JavaScript syntax check: $($_.Name)"
        }

    Write-Host "TempleFix source checks passed."
}
finally {
    Pop-Location
}
