@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build_windows.ps1"
set "build_exit_code=%ERRORLEVEL%"
echo.
if "%build_exit_code%"=="0" (
    echo Windows build finished. Results are listed above.
) else (
    echo Windows build failed with exit code %build_exit_code%. See the error above.
)
rem Keep the results visible when launched by double-click; skip in CI.
if not defined CI pause
exit /b %build_exit_code%
