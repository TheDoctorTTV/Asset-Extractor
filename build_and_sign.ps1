Write-Host "Starting PyInstaller Build..." -ForegroundColor Cyan

# Build Command (as single string, backticks for line continuation)
$buildCmd = "pyinstaller --clean --onefile --noconsole --icon='G:\Git Repos\Beat Saber Mapping Extractor\beat-saber-map-extractor-icon.ico' " +
"--add-data 'C:/Users/minec/AppData/Local/Programs/Python/Python311/Lib/site-packages/UnityPy/resources;UnityPy/resources' " +
"--add-data 'G:/Git Repos/Beat Saber Mapping Extractor/beat-saber-map-extractor-icon.ico;.' " +
"--upx-dir 'G:\upx-5.0.1-win64' --noupx " +
"--exclude-module torch " +
"--exclude-module scipy " +
"--exclude-module pandas " +
"--exclude-module matplotlib " +
"--exclude-module IPython " +
"--exclude-module jinja2 " +
"--exclude-module numba " +
"--exclude-module PIL.SpiderImagePlugin " +
"Beat-Saber-Map-Extractor.py"

# Run Build
Invoke-Expression $buildCmd

if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host "Build completed successfully!" -ForegroundColor Green

# Signing Command
Write-Host "Signing the EXE..." -ForegroundColor Cyan

$signCmd = "signtool sign /tr http://timestamp.digicert.com /td SHA256 /fd SHA256 /a /f 'G:\OneDrive\Desktop\misc\TheDoctorTTV.cer' /p 6tsh9c59dpr7 'G:\Git Repos\Beat Saber Mapping Extractor\dist\Beat-Saber-Map-Extractor.exe'"

# Run Signing
Invoke-Expression $signCmd

if ($LASTEXITCODE -ne 0) {
    Write-Error "Signing failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host 'Build & Signing Completed Successfully!' -ForegroundColor Green
