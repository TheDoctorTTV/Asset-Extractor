param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') {
    throw 'Run build_windows.ps1 on Windows. On Linux use build_linux.sh.'
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'Cargo is missing. Install stable Rust using rustup, then reopen your terminal.'
}
Push-Location $PSScriptRoot
try {
    & cargo build --release --target-dir (Join-Path $PSScriptRoot 'target')
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed with exit code $LASTEXITCODE" }
    $destination = Join-Path $PSScriptRoot 'dist/windows'
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath 'target/release/beat-saber-mapping-extractor.exe' -Destination $destination -Force
    Copy-Item -LiteralPath 'README.md' -Destination $destination -Force
    Write-Host "Built: $destination/beat-saber-mapping-extractor.exe"
}
finally { Pop-Location }
