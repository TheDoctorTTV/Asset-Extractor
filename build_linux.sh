#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
if [[ "$(uname -s)" != Linux ]]; then
    echo "Run build_linux.sh on Linux. On Windows use build_windows.bat." >&2
    exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "Cargo is missing. Install stable Rust using rustup, then reopen your terminal." >&2
    exit 1
fi
# Use an explicit local artifact directory, independent of user Cargo configuration.
cargo build --release --target-dir "$PWD/target"
mkdir -p dist/linux
install -m 755 target/release/beat-saber-mapping-extractor dist/linux/beat-saber-mapping-extractor
cp README.md dist/linux/README.md
echo "Built: $PWD/dist/linux/beat-saber-mapping-extractor"
