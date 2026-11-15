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
    $buildBinary = [regex]::Match($manifestText, '(?m)^name = "([^"]+)"').Groups[1].Value
    if (-not $buildTarget -or -not $buildVersion -or -not $buildBinary) { throw 'Could not determine the build target, package name, or version.' }
    & cargo build --release --bin $buildBinary --target $buildTarget --target-dir (Join-Path $PSScriptRoot 'target')
    if ($LASTEXITCODE -ne 0) { throw "Cargo build failed with exit code $LASTEXITCODE" }
    $destination = Join-Path $PSScriptRoot 'dist/windows'
    New-Item -ItemType Directory -Force -Path $destination | Out-Null
    Copy-Item -LiteralPath 'README.md', 'docs.md' -Destination $destination -Force
    $releaseArch = switch ($buildTarget) {
        'x86_64-pc-windows-msvc' { 'x86_64' }
        'aarch64-pc-windows-msvc' { 'aarch64' }
        default { $null }
    }
    if ($releaseArch) {
        $releaseName = "asset-extractor-v$buildVersion-windows-$releaseArch.exe"
        $releaseFile = Join-Path $destination $releaseName
        Copy-Item -LiteralPath "target/$buildTarget/release/$buildBinary.exe" -Destination $releaseFile -Force
        $legacyFile = Join-Path $destination 'asset-extractor.exe'
        if (Test-Path -LiteralPath $legacyFile) { Remove-Item -LiteralPath $legacyFile -Force }
        $checksum = (Get-FileHash -LiteralPath $releaseFile -Algorithm SHA256).Hash.ToLowerInvariant()
        [IO.File]::WriteAllText("$releaseFile.sha256", "$checksum  $releaseName`n", [Text.Encoding]::ASCII)
    } else {
        $releaseFile = Join-Path $destination 'asset-extractor.exe'
        Copy-Item -LiteralPath "target/$buildTarget/release/$buildBinary.exe" -Destination $releaseFile -Force
    }
}
finally { Pop-Location }

Write-Host ''
Write-Host 'Windows build completed successfully.'
Write-Host "Target:     $buildTarget"
Write-Host "Folder:     $destination"
Write-Host "Executable: $releaseFile"
if ($releaseArch) {
    Write-Host "Checksum:   $releaseFile.sha256"
    Write-Host "SHA-256:    $checksum"
} else {
    Write-Host 'Checksum:   Not generated for this unsupported updater target.'
}
Write-Host "README:     $(Join-Path $destination 'README.md')"
