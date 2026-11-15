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
    $rustVersionInfo = & rustc -vV
    if ($LASTEXITCODE -ne 0) { throw 'Could not determine the Rust host target.' }
    $buildTarget = ($rustVersionInfo | Where-Object { $_ -match '^host: ' }) -replace '^host: ', ''
    $manifestText = Get-Content -LiteralPath 'Cargo.toml' -Raw
    $buildVersion = [regex]::Match($manifestText, '(?m)^version = "([^"]+)"').Groups[1].Value
    if (-not $buildTarget -or -not $buildVersion) { throw 'Could not determine the build target or version.' }
    & cargo build --release --target $buildTarget --target-dir (Join-Path $PSScriptRoot 'target')
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed with exit code $LASTEXITCODE" }
    $destination = Join-Path $PSScriptRoot 'dist/windows'
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath "target/$buildTarget/release/beat-saber-mapping-extractor.exe" -Destination $destination -Force
    Copy-Item -LiteralPath 'README.md' -Destination $destination -Force
    Write-Host "Built: $destination/beat-saber-mapping-extractor.exe"
    $releaseArch = switch ($buildTarget) {
        'x86_64-pc-windows-msvc' { 'x86_64' }
        'aarch64-pc-windows-msvc' { 'aarch64' }
        default { $null }
    }
    if ($releaseArch) {
        $releaseDirectory = Join-Path $PSScriptRoot 'dist/releases'
        New-Item -ItemType Directory -Force -Path $releaseDirectory | Out-Null
        $releaseName = "beat-saber-mapping-extractor-v$buildVersion-windows-$releaseArch.exe"
        $releaseFile = Join-Path $releaseDirectory $releaseName
        Copy-Item -LiteralPath (Join-Path $destination 'beat-saber-mapping-extractor.exe') -Destination $releaseFile -Force
        $checksum = (Get-FileHash -LiteralPath $releaseFile -Algorithm SHA256).Hash.ToLowerInvariant()
        [IO.File]::WriteAllText("$releaseFile.sha256", "$checksum  $releaseName`n", [Text.Encoding]::ASCII)
        Write-Host "GitHub release assets: $releaseFile and .sha256"
    } else {
        Write-Host "No in-app updater asset generated for $buildTarget."
    }
}
finally { Pop-Location }
