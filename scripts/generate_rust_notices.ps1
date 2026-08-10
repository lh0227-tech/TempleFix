param(
    [string]$ManifestPath = "src-tauri\Cargo.toml",
    [string]$OutputPath = "THIRD_PARTY_LICENSES.txt"
)

$ErrorActionPreference = "Stop"
$WorkspaceRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$ManifestPath = [System.IO.Path]::GetFullPath((Join-Path $WorkspaceRoot $ManifestPath))
$OutputPath = [System.IO.Path]::GetFullPath((Join-Path $WorkspaceRoot $OutputPath))
if (-not $OutputPath.StartsWith($WorkspaceRoot + [System.IO.Path]::DirectorySeparatorChar, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to write third-party notices outside the workspace."
}

$metadataJson = cargo.exe metadata --format-version 1 --filter-platform x86_64-pc-windows-msvc --manifest-path $ManifestPath
if ($LASTEXITCODE -ne 0) {
    throw "cargo metadata failed."
}
$metadata = $metadataJson | ConvertFrom-Json
$resolvedIds = @{}
foreach ($node in $metadata.resolve.nodes) {
    $resolvedIds[$node.id] = $true
}

$builder = New-Object System.Text.StringBuilder
[void]$builder.AppendLine("TempleFix third-party software notices")
[void]$builder.AppendLine("Generated from Cargo metadata for x86_64-pc-windows-msvc.")
[void]$builder.AppendLine("TempleFix is not affiliated with or endorsed by these projects.")
[void]$builder.AppendLine()

$packages = $metadata.packages |
    Where-Object { $resolvedIds.ContainsKey($_.id) -and $_.name -ne "templefix" } |
    Sort-Object name, version

function Get-PackageLicenseFiles($Package) {
    $packageRoot = Split-Path -Parent $Package.manifest_path
    $files = @()
    if ($Package.license_file) {
        $declared = Join-Path $packageRoot $Package.license_file
        if (Test-Path -LiteralPath $declared -PathType Leaf) {
            $files += Get-Item -LiteralPath $declared
        }
    }
    $files += Get-ChildItem -LiteralPath $packageRoot -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)(\.|-|$)' }
    return @($files | Sort-Object FullName -Unique)
}

$mitTemplate = @"
Copyright (c) the listed authors and contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"@

$bsd3Template = @"
Copyright (c) the listed authors and contributors
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice,
   this list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
"@

# Reuse the standard MPL-2.0 text shipped by another resolved MPL dependency
# when a crate from the same dependency graph omitted its standalone file.
$mpl2Template = $null
foreach ($candidate in $packages | Where-Object { $_.license -match 'MPL-2\.0' }) {
    foreach ($file in (Get-PackageLicenseFiles $candidate)) {
        $content = [System.IO.File]::ReadAllText($file.FullName)
        if ($content -match 'Mozilla Public License Version 2\.0') {
            $mpl2Template = $content
            break
        }
    }
    if ($mpl2Template) { break }
}

foreach ($package in $packages) {
    [void]$builder.AppendLine(("=" * 78))
    [void]$builder.AppendLine(("{0} {1}" -f $package.name, $package.version))
    [void]$builder.AppendLine(("License expression: {0}" -f $package.license))
    if ($package.repository) {
        [void]$builder.AppendLine(("Repository: {0}" -f $package.repository))
    }
    if ($package.authors -and $package.authors.Count -gt 0) {
        [void]$builder.AppendLine(("Authors: {0}" -f ($package.authors -join "; ")))
    }

    $licenseFiles = Get-PackageLicenseFiles $package

    if ($licenseFiles.Count -eq 0) {
        [void]$builder.AppendLine()
        [void]$builder.AppendLine("--- Standard license text (crate source omitted a standalone file) ---")
        if ($package.license -match 'MIT') {
            [void]$builder.AppendLine($mitTemplate)
        } elseif ($package.license -match 'BSD-3-Clause') {
            [void]$builder.AppendLine($bsd3Template)
        } elseif ($package.license -match 'MPL-2\.0' -and $mpl2Template) {
            [void]$builder.AppendLine($mpl2Template)
        } else {
            [void]$builder.AppendLine("Refer to the license expression and upstream repository listed above.")
        }
    } else {
        foreach ($file in $licenseFiles) {
            [void]$builder.AppendLine()
            [void]$builder.AppendLine(("--- {0} ---" -f $file.Name))
            [void]$builder.AppendLine([System.IO.File]::ReadAllText($file.FullName))
        }
    }
    [void]$builder.AppendLine()
}

$encoding = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($OutputPath, $builder.ToString(), $encoding)
Get-Item -LiteralPath $OutputPath | Select-Object FullName, Length
