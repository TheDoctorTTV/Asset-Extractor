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
build_binary="$(sed -n 's/^name = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
if [[ -z "$build_binary" || -z "$build_version" ]]; then
    echo "Could not determine the package name or version from Cargo.toml." >&2
    exit 1
fi
case "$build_target" in
    x86_64-unknown-linux-gnu) release_arch=x86_64 ;;
    aarch64-unknown-linux-gnu) release_arch=aarch64 ;;
    *) echo "AppImage packaging is unsupported for $build_target." >&2; exit 1 ;;
esac

# Cache the packaging tool separately from the generated distribution.
linuxdeploy="$PWD/target/appimage-tools/linuxdeploy-${release_arch}.AppImage"
if [[ ! -x "$linuxdeploy" ]]; then
    if ! command -v curl >/dev/null 2>&1; then
        echo "curl is required to download linuxdeploy for AppImage packaging." >&2
        exit 1
    fi
    mkdir -p -- "$(dirname -- "$linuxdeploy")"
    curl --fail --location --retry 3 \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${release_arch}.AppImage" \
        --output "$linuxdeploy.download"
    chmod 755 -- "$linuxdeploy.download"
    mv -f -- "$linuxdeploy.download" "$linuxdeploy"
fi

cargo build --release --bin "$build_binary" --target "$build_target" --target-dir "$PWD/target"
mkdir -p dist/linux
# The executable remains an intermediate Cargo artifact, never a distribution asset.
build_executable="$PWD/target/$build_target/release/$build_binary"
# Start with a fresh AppDir so removed libraries cannot leak into later builds.
app_id=io.github.TheDoctorTTV.BeatSaberMappingExtractor
release_name="beat-saber-mapping-extractor-v${build_version}-linux-${release_arch}"
appimage_path="$PWD/dist/linux/$release_name.AppImage"
icon_path="$PWD/assets/$app_id.png"
app_dir="$(mktemp -d "$PWD/dist/linux/.AppDir.XXXXXX")"
trap 'rm -rf -- "$app_dir"' EXIT
# mktemp creates private directories; the AppImage root must be readable by everyone.
chmod 755 -- "$app_dir"
mkdir -p -- "$app_dir/usr/share/applications" "$app_dir/usr/share/doc/beat-saber-mapping-extractor"
cp README.md "$app_dir/usr/share/doc/beat-saber-mapping-extractor/README.md"
cat > "$app_dir/usr/share/applications/$app_id.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Asset Extractor
Comment=Extract Beat Saber bundle assets and map data
Exec=$build_binary
Icon=$app_id
Terminal=false
Categories=Utility;
StartupNotify=true
StartupWMClass=beat-saber-mapping-extractor
EOF
# Extract the packaging tools when running them, avoiding a build-time FUSE requirement.
# linuxdeploy bundles eligible shared libraries and supplies the AppRun launcher.
# appimagetool writes the zsync sidecar in its working directory.
(
cd -- "$(dirname -- "$appimage_path")"
ARCH="$release_arch" APPIMAGE_EXTRACT_AND_RUN=1 \
    LDAI_OUTPUT="$appimage_path" LINUXDEPLOY_OUTPUT_VERSION="$build_version" \
    LDAI_UPDATE_INFORMATION="gh-releases-zsync|TheDoctorTTV|Beat-Saber-Mapping-Extractor|latest|beat-saber-mapping-extractor-v*-linux-${release_arch}.AppImage.zsync" \
    "$linuxdeploy" --appdir "$app_dir" \
        --executable "$build_executable" \
        --desktop-file "$app_dir/usr/share/applications/$app_id.desktop" \
        --icon-file "$icon_path" \
        --output appimage
)
if [[ ! -s "$appimage_path.zsync" ]]; then
    echo "AppImage packaging did not produce the required zsync update file." >&2
    exit 1
fi
chmod 755 -- "$appimage_path"
echo "Built AppImage: $appimage_path"

(cd dist/linux && sha256sum "$release_name.AppImage" > "$release_name.AppImage.sha256")
echo "AppImage release assets: $appimage_path, .AppImage.sha256 and .AppImage.zsync"
