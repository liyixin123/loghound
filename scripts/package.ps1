<#
.SYNOPSIS
    Package loghound: build release, collect exe + config + deploy scripts
    into dist\, and compress to a zip.
.EXAMPLE
    powershell -ExecutionPolicy Bypass -File scripts\package.ps1
.NOTES
    Kept ASCII-only on purpose: Windows PowerShell 5.1 reads .ps1 files as the
    system ANSI codepage when there is no BOM, so non-ASCII text would break parsing.
#>
[CmdletBinding()]
param(
    # Skip cargo build and package the existing target\release\loghound.exe
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'

# Repo root (this script lives in scripts\)
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

# Read version from Cargo.toml
$version = (Select-String -Path (Join-Path $Root 'Cargo.toml') -Pattern '^version\s*=\s*"(.+)"').Matches[0].Groups[1].Value
Write-Host "loghound version: $version" -ForegroundColor Cyan

if (-not $NoBuild) {
    Write-Host "Building release..." -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$exe = Join-Path $Root 'target\release\loghound.exe'
if (-not (Test-Path $exe)) { throw "Not found: $exe (build first)" }

# Prepare dist\loghound (top level) and dist\loghound\app (everything else)
$distRoot = Join-Path $Root 'dist'
$payload = Join-Path $distRoot 'loghound'
$app = Join-Path $payload 'app'
if (Test-Path $payload) { Remove-Item $payload -Recurse -Force }
New-Item -ItemType Directory -Path $app -Force | Out-Null

# Top level: only the two entry points (and a short readme)
Copy-Item (Join-Path $PSScriptRoot 'install.bat')   $payload
Copy-Item (Join-Path $PSScriptRoot 'uninstall.bat') $payload
Copy-Item (Join-Path $Root 'README.txt')            $payload

# app\ subfolder: exe, config, hidden launcher (kept out of top level to avoid mis-clicks)
Copy-Item $exe                                 (Join-Path $app 'loghound.exe')
Copy-Item (Join-Path $Root 'loghound.toml')    (Join-Path $app 'loghound.toml')
Copy-Item (Join-Path $PSScriptRoot 'run-hidden.vbs') $app

# Compress
$zip = Join-Path $distRoot "loghound-$version.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $payload '*') -DestinationPath $zip

Write-Host ""
Write-Host "Package done:" -ForegroundColor Green
Write-Host "  folder: $payload"
Write-Host "  zip:    $zip"
Write-Host ""
Write-Host "Deploy: unzip, then run install.bat (auto-elevates)." -ForegroundColor Yellow
