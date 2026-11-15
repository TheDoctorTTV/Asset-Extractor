#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"

if [[ "$(uname -s)" != Linux ]]; then
    echo "Run build_all.sh on Linux. On Windows use build_windows.bat." >&2
    exit 1
fi
if [[ $# -gt 1 || ( $# -eq 1 && "$1" != x86_64 && "$1" != aarch64 ) ]]; then
    echo "Usage: $0 [x86_64|aarch64] (optional Windows architecture)" >&2
    exit 1
fi

echo "Building Linux AppImage..."
if bash ./build_linux.sh; then
    echo "Linux build completed."
else
    build_exit_code=$?
    echo "Linux build failed (exit $build_exit_code). Windows build was not started." >&2
    exit "$build_exit_code"
fi

echo
echo "Building Windows executable..."
if bash ./build_windows.sh "$@"; then
    echo
    echo "Both builds completed successfully."
    echo "Linux results:   $PWD/dist/linux"
    echo "Windows results: $PWD/dist/windows"
else
    build_exit_code=$?
    echo "Windows build failed (exit $build_exit_code). Linux results remain in $PWD/dist/linux." >&2
    exit "$build_exit_code"
fi
