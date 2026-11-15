#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "${BASH_SOURCE[0]}")"

if [[ "$(uname -s)" != Linux ]]; then
    echo "Run build_windows.sh on Linux. On Windows use build_windows.bat." >&2
    exit 1
fi
release_arch="${1:-x86_64}"
if [[ $# -gt 1 ]]; then
    echo "Usage: $0 [x86_64|aarch64]" >&2
    exit 1
fi
case "$release_arch" in
    x86_64|aarch64) build_target="${release_arch}-pc-windows-msvc" ;;
    *) echo "Usage: $0 [x86_64|aarch64]" >&2; exit 1 ;;
esac

for tool in cargo rustc rustup; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "$tool is missing. Install stable Rust using rustup, then reopen your terminal." >&2
        exit 1
    fi
done
if ! command -v cargo-xwin >/dev/null 2>&1; then
    echo "cargo-xwin is missing. Install it with: cargo install --locked cargo-xwin" >&2
    exit 1
fi
# Debian exposes clang-cl with a version suffix in /usr/bin, while LLVM's
# private bin directory contains the unversioned names cargo-xwin needs.
if ! command -v clang-cl >/dev/null 2>&1 || ! command -v lld-link >/dev/null 2>&1; then
    clang_bin_dir=""
    if command -v clang >/dev/null 2>&1; then
        clang_bin_dir="$(dirname -- "$(readlink -f -- "$(command -v clang)")")"
    fi
    for llvm_bin_dir in "$clang_bin_dir" /usr/lib/llvm-*/bin; do
        if [[ -x "$llvm_bin_dir/clang-cl" && -x "$llvm_bin_dir/lld-link" ]]; then
            export PATH="$llvm_bin_dir:$PATH"
            break
        fi
    done
fi
for tool in clang-cl lld-link; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "$tool is missing from PATH. Install Clang/LLVM and LLD." >&2
        if command -v apt-get >/dev/null 2>&1; then
            echo "On Debian/Ubuntu: sudo apt install build-essential clang llvm lld" >&2
        elif command -v pacman >/dev/null 2>&1; then
            echo "On CachyOS/Arch: sudo pacman -S --needed clang llvm lld" >&2
        fi
        exit 1
    fi
done
if ! command -v sha256sum >/dev/null 2>&1; then
    echo "sha256sum is missing. Install coreutils." >&2
    exit 1
fi
installed_targets="$(rustup target list --installed)"
if ! [[ $'\n'"$installed_targets"$'\n' == *$'\n'"$build_target"$'\n'* ]]; then
    echo "Rust target is missing. Install it with: rustup target add $build_target" >&2
    exit 1
fi
build_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
build_binary="$(sed -n 's/^name = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)"
if [[ -z "$build_binary" || -z "$build_version" ]]; then
    echo "Could not determine the package name or version from Cargo.toml." >&2
    exit 1
fi

# cargo-xwin supplies the Microsoft SDK and CRT for the Windows MSVC target.
cargo xwin build --release --bin "$build_binary" --target "$build_target" --target-dir "$PWD/target"
mkdir -p dist/windows
release_name="beat-saber-mapping-extractor-v${build_version}-windows-${release_arch}.exe"
install -m 755 "target/$build_target/release/$build_binary.exe" "dist/windows/$release_name"
# Remove the redundant filename left by earlier versions of this script.
rm -f -- dist/windows/beat-saber-mapping-extractor.exe
cp README.md dist/windows/README.md
(cd dist/windows && sha256sum "$release_name" > "$release_name.sha256")
echo
echo "Windows build completed successfully."
echo "Target:     $build_target"
echo "Folder:     $PWD/dist/windows"
echo "Executable: $PWD/dist/windows/$release_name"
echo "Checksum:   $PWD/dist/windows/$release_name.sha256"
read -r checksum _ < "dist/windows/$release_name.sha256"
echo "SHA-256:    $checksum"
echo "README:     $PWD/dist/windows/README.md"
