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
# Build for this Rust toolchain's host, independent of Cargo target overrides.
build_target="$(rustc -vV | sed -n 's/^host: //p')"
build_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
cargo build --release --target "$build_target" --target-dir "$PWD/target"
mkdir -p dist/linux
install -m 755 "target/$build_target/release/beat-saber-mapping-extractor" dist/linux/beat-saber-mapping-extractor
cp README.md dist/linux/README.md
cp assets/io.github.TheDoctorTTV.BeatSaberMappingExtractor.png dist/linux/
install -m 755 packaging/linux/install_desktop.sh dist/linux/install_desktop.sh
if [[ "${INSTALL_DESKTOP_ENTRY:-1}" != 0 ]]; then
    if ! bash dist/linux/install_desktop.sh; then
        echo "Desktop registration failed; the binary was built. Rerun dist/linux/install_desktop.sh to register the icon." >&2
    fi
fi
echo "Built: $PWD/dist/linux/beat-saber-mapping-extractor"

case "$build_target" in
    x86_64-unknown-linux-gnu) release_arch=x86_64 ;;
    aarch64-unknown-linux-gnu) release_arch=aarch64 ;;
    *) echo "No in-app updater asset generated for $build_target."; exit 0 ;;
esac
mkdir -p dist/releases
release_name="beat-saber-mapping-extractor-v${build_version}-linux-${release_arch}"
install -m 755 dist/linux/beat-saber-mapping-extractor "dist/releases/$release_name"
(cd dist/releases && sha256sum "$release_name" > "$release_name.sha256")
echo "GitHub release assets: $PWD/dist/releases/$release_name and .sha256"
