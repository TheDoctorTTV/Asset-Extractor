param(
    [string]$CertPath = "C:\Users\minec\Desktop\misc\TheDoctorTTV.cer", # Default to your .cer; pass a .pfx if available
    [SecureString]$CertPassword,                                           # SecureString param for interactive/secure input
    [string]$CertPasswordPlain = $env:SIGN_CERT_PASSWORD,                  # Optional: env var fallback (plain text)
    [switch]$CleanExeOnly                                                  # Only delete the EXE (default behavior)
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Write-Host "Starting PyInstaller Build..." -ForegroundColor Cyan

# Helper to remove a file with retries, attempt to stop any process locking it
function Remove-FileRobust {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [int]$Retries = 5,
        [int]$DelayMs = 600
    )
    if (-not (Test-Path $Path)) { return }
    for ($i = 0; $i -lt $Retries; $i++) {
        try {
            # Attempt to stop any running process that matches the path
            try {
                $procs = Get-Process | Where-Object { $_.Path -eq $Path } | Select-Object -Expand Id -ErrorAction SilentlyContinue
                if ($procs) {
                    Write-Host "Stopping process locking EXE: $procs" -ForegroundColor Yellow
                    $procs | ForEach-Object { Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue }
                }
            }
            catch {}

            # Remove ReadOnly attribute if set
            try {
                $item = Get-Item -LiteralPath $Path -ErrorAction SilentlyContinue
                if ($item -and $item.Attributes -band [IO.FileAttributes]::ReadOnly) {
                    $item.Attributes = $item.Attributes -bxor [IO.FileAttributes]::ReadOnly
                }
            }
            catch {}

            Remove-Item -LiteralPath $Path -Force -ErrorAction Stop
            return
        }
        catch {
            if ($i -lt ($Retries - 1)) {
                Start-Sleep -Milliseconds $DelayMs
            }
            else {
                throw
            }
        }
    }
}

# Paths
$ProjectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$DistDir = Join-Path $ProjectRoot 'dist'
$ExePath = Join-Path $DistDir 'Beat-Saber-Map-Extractor.exe'

# Delete old EXE before building fresh
if (Test-Path $ExePath) {
    Write-Host "Removing old EXE: $ExePath" -ForegroundColor Yellow
    Remove-FileRobust -Path $ExePath
}

# Optionally ensure dist dir exists/clean (we keep to user's request: only EXE)
if (-not (Test-Path $DistDir)) { New-Item -ItemType Directory -Path $DistDir | Out-Null }

# Build args (use array to avoid quoting issues)
$pyArgs = @(
    '--clean', '--onefile', '--noconsole',
    "--icon=G:\Git Repos\Beat Saber Mapping Extractor\beat-saber-map-extractor-icon.ico",
    '--add-data', 'C:/Users/minec/AppData/Local/Programs/Python/Python311/Lib/site-packages/UnityPy/resources;UnityPy/resources',
    '--add-data', 'G:/Git Repos/Beat Saber Mapping Extractor/beat-saber-map-extractor-icon.ico;.',
    '--upx-dir', 'G:\upx-5.0.1-win64', '--noupx',
    '--exclude-module', 'torch',
    '--exclude-module', 'scipy',
    '--exclude-module', 'pandas',
    '--exclude-module', 'matplotlib',
    '--exclude-module', 'IPython',
    '--exclude-module', 'jinja2',
    '--exclude-module', 'numba',
    '--exclude-module', 'PIL.SpiderImagePlugin',
    'Beat-Saber-Map-Extractor.py'
)

& pyinstaller @pyArgs

if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host "Build completed successfully!" -ForegroundColor Green

# Signing
Write-Host "Signing the EXE..." -ForegroundColor Cyan

if (-not (Test-Path $ExePath)) {
    Write-Error "Built EXE not found at $ExePath"
    exit 1
}

# Build signtool argument list
$tsUrl = 'http://timestamp.digicert.com'
$signArgs = @('sign', '/tr', $tsUrl, '/td', 'SHA256', '/fd', 'SHA256')

if (Test-Path $CertPath) {
    if ($CertPath.ToLower().EndsWith('.cer')) {
        # Use the .cer to get the thumbprint, then locate the matching cert with a private key in the store
        try {
            $cerObj = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($CertPath)
            $thumb = ($cerObj.Thumbprint).Replace(' ', '')
        }
        catch {
            Write-Warning "Failed to read .cer at ${CertPath}: $($_.Exception.Message). Falling back to /a."
            $thumb = $null
        }

        if ($thumb) {
            $storePaths = @('Cert:\CurrentUser\My', 'Cert:\LocalMachine\My')
            $storeMatch = $null
            foreach ($sp in $storePaths) {
                try {
                    $candidate = Get-ChildItem $sp | Where-Object { $_.Thumbprint -eq $thumb -and $_.HasPrivateKey } | Select-Object -First 1
                    if ($candidate) { $storeMatch = @{ Path = $sp; Cert = $candidate }; break }
                }
                catch {}
            }
            if ($storeMatch) {
                Write-Host "Using cert from store: $($storeMatch.Path) with thumbprint $thumb" -ForegroundColor Cyan
                $signArgs += @('/sha1', $thumb, '/s', 'my')
                if ($storeMatch.Path -like 'Cert:\LocalMachine*') { $signArgs += '/sm' }
            }
            else {
                Write-Warning "No matching certificate with a private key found in CurrentUser/LocalMachine 'My' store for thumbprint $thumb."
                Write-Warning "Import the corresponding PFX into the certificate store or provide a .pfx via -CertPath. Falling back to /a."
                $signArgs += '/a'
            }
        }
        else {
            $signArgs += '/a'
        }
    }
    else {
        # PFX path supplied: sign directly using file and optional password
        $signArgs += @('/f', $CertPath)
        $pwdPlain = $null
        if ($CertPassword) {
            $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($CertPassword)
            try { $pwdPlain = [Runtime.InteropServices.Marshal]::PtrToStringAuto($bstr) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
        }
        elseif (-not [string]::IsNullOrWhiteSpace($CertPasswordPlain)) {
            $pwdPlain = $CertPasswordPlain
        }
        else {
            # Prompt for password if not provided and required
            $sec = Read-Host -AsSecureString -Prompt 'Enter PFX password (input hidden)'
            $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($sec)
            try { $pwdPlain = [Runtime.InteropServices.Marshal]::PtrToStringAuto($bstr) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
        }
        if (-not [string]::IsNullOrWhiteSpace($pwdPlain)) {
            $signArgs += @('/p', $pwdPlain)
        }
    }
}
else {
    Write-Warning "CertPath not found: $CertPath. Using /a to select from certificate store."
    $signArgs += '/a'
}

$signArgs += $ExePath

& signtool @signArgs

if ($LASTEXITCODE -ne 0) {
    Write-Error "Signing failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

Write-Host 'Build & Signing Completed Successfully!' -ForegroundColor Green
