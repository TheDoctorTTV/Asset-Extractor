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
cp assets/io.github.TheDoctorTTV.BeatSaberMappingExtractor.png dist/linux/
install -m 755 packaging/linux/install_desktop.sh dist/linux/install_desktop.sh
if [[ "${INSTALL_DESKTOP_ENTRY:-1}" != 0 ]]; then
    if ! bash dist/linux/install_desktop.sh; then
        echo "Desktop registration failed; the binary was built. Rerun dist/linux/install_desktop.sh to register the icon." >&2
    fi
fi
echo "Built: $PWD/dist/linux/beat-saber-mapping-extractor"
