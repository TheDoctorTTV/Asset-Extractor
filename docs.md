# Asset Extractor documentation

[Project overview and quick start](README.md)

## Extraction and output

Extraction runs in a background worker. Home lets you select a bundle, cancel extraction, and open the result. Console shows progress, filenames, and warnings, retains the latest 2,000 lines, and provides a Copy log button.

The backend is `unity-rs-core` 0.5.0. Export behavior depends on the asset and codec:

| Asset | Output |
| --- | --- |
| TextAsset | Original bytes; gzip payloads expand into `.dat` files |
| Supported texture | PNG |
| Supported audio | WAV |
| Other supported object | Backend-selected format |
| Unsupported object | Raw `.bin` where possible, with a warning |
| Unrecognized audio codec | Original audio container |

Raw fallback files are not decoded media. Missing metadata and unsupported formats can prevent an extraction from being a playable map. Review Console warnings.

### Existing folders and recovery

Each extraction uses a folder named after the bundle. If it already exists, choose a new folder name, overwrite, or cancel. Filenames are made portable and duplicate names receive numbered suffixes.

Files are staged before saving. Overwrite preserves the previous folder until new assets are prepared, then temporarily backs it up. If saving fails, the app attempts restoration and reports any retained backup path. A successful replacement removes the backup. Overwrite replaces the whole folder, including files you added yourself.

Cancellation takes effect between decoder stages and during conversion; an active backend stage must finish first. Closing the app requests cancellation without waiting for the worker. Forced termination or power loss can leave staging, backup, or partial output folders.

Default output limits are 512 MiB per prepared file and 16 GiB total, alongside backend parsing and decoding limits.

## Beat Saber metadata

The generated v4.0.1 `Info.dat` references exported audio, preview, cover, audio timing, beatmaps, and lightshows. When a bundled v4 info file exists, its song metadata, difficulty settings, credits, environments, and color schemes are reused. The original is preserved under a numbered name such as `Info-2.dat`.

Built-in song environments require the original `StreamingAssets/BeatmapLevelsData` bundle and sibling `StreamingAssets/aa/<platform>` pack and shared-assets bundles. A copied gameplay bundle alone does not contain these settings. Matching metadata supplies difficulty environment overrides and 90°/360° defaults.

Without bundled metadata:

- Difficulty and characteristic names are inferred from filenames. Unqualified names use Standard; unknown difficulties are omitted with a warning.
- Lightshows prefer a matching difficulty, then a characteristic match, `Common.lightshow.dat`, or a single shared lightshow.
- BPM and duration use AudioData timing where available. The complete timing payload is preserved as `AudioData.dat`, the literal filename required by the official editor’s working-folder copy.
- Missing values fall back to the bundle name for title, 120 BPM, zero duration and jump settings, `DefaultEnvironment`, and empty credits. Console reports missing and inferred values.

Schema references: [Info format](https://bsmg.wiki/mapping/map-format/info.html) and [audio timing](https://bsmg.wiki/mapping/map-format/audio.html).

## Settings and updates

Themes apply and save immediately. Default output-folder changes require **Save settings**. Settings use the current user’s configuration directory through `directories::ProjectDirs`, with organization `TheDoctorTTV` and application `BeatSaberMappingExtractor`.

About offers release and pre-release channels, manual checks, and checks at startup or hourly, daily, every 48 hours, or weekly while the app remains open. A red About badge indicates a newer compatible release. Installing, reinstalling, or downgrading requires an explicit action and is disabled during extraction.

Executable updates use this project’s public GitHub Releases API without a token. Downloads must match their published size and SHA-256 digest or checksum, and pass OS/architecture checks. Invalid tags, drafts, and versions below 1.0.0 are excluded. Supported updater targets are x86_64 and aarch64 Linux GNU and Windows MSVC.

Replacement requires a writable executable directory. Settings and extracted files are retained. Installation failures report a backup path where available; restart failures require reopening manually. There is no startup health check or automatic rollback after launch.

### AppImages and Gear Lever

The executable updater cannot replace the outer AppImage. Use Gear Lever or download a replacement AppImage. At startup, the app checks native and Flatpak Gear Lever configuration for the running AppImage’s registered path. A match hides updater controls and badges and disables built-in checks. Restart after changing registration.

AppImages embed an architecture-specific `gh-releases-zsync` source for the latest stable release. This source does not select pre-releases.

Extraction works offline. Release checks and downloads require internet access. About also fetches the GitHub avatar when opened, refreshing after an hour; the image is held in memory rather than saved to disk.

## Build prerequisites

Use current stable Rust through rustup. The extraction backend requires at least Rust 1.88; other dependencies may require newer versions. Cargo downloads dependencies as needed. Keep `Cargo.lock` tracked.

### Linux AppImage

Install a C/C++ toolchain, pkg-config, curl, and display development libraries:

```bash
# Debian / Ubuntu
sudo apt install build-essential pkg-config curl libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxinerama-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev

# Fedora
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config curl libX11-devel libXi-devel libXcursor-devel libXrandr-devel libXinerama-devel libxkbcommon-devel wayland-devel mesa-libGL-devel

# Arch / CachyOS
sudo pacman -S --needed base-devel pkgconf curl libx11 libxi libxcursor libxrandr libxinerama libxkbcommon wayland mesa
```

Run `./build_linux.sh`. It downloads and caches linuxdeploy under `target/appimage-tools`; packaging does not require FUSE. Supported hosts are x86_64 and aarch64 GNU Linux. Build on the oldest distribution you intend to support: AppImages still require compatible host glibc and graphics drivers.

### Windows on Windows

Install Rust’s MSVC toolchain and Visual Studio Build Tools with **Desktop development with C++** and a Windows SDK. Reopen the terminal, then run `build_windows.bat` or `./build_windows.ps1`.

The batch launcher applies a process-local PowerShell execution-policy override and pauses at the end. Set `CI` to skip the pause. It preserves the build exit code.

### Windows from Linux

Install Clang, LLVM, LLD, and a C/C++ toolchain inside the environment where you build:

```bash
# Debian / Ubuntu
sudo apt install build-essential clang llvm lld

# Arch / CachyOS
sudo pacman -S --needed base-devel clang llvm lld

cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
./build_windows.sh
```

The script finds Debian’s versioned LLVM directory automatically when `clang-cl` is absent from PATH. For ARM64, install `aarch64-pc-windows-msvc` and run `./build_windows.sh aarch64`.

[cargo-xwin](https://github.com/rust-cross/cargo-xwin) downloads and caches Microsoft’s SDK and CRT on first use; its documented Microsoft license applies. Wine is unnecessary for compilation.

To run both builds inside an existing Distrobox, enter it, change to the repository, and run `./build_all.sh`. Install prerequisites inside that box. The combined script builds Linux first, then Windows, stopping on failure.

## Troubleshooting

- **AppImage cannot mount:** run it with `--appimage-extract-and-run` if FUSE is unavailable.
- **File dialog does not open:** install `xdg-desktop-portal` and the backend for your desktop, such as KDE or GTK.
- **Missing `cargo-xwin` or Windows target:** use the installation commands above. LLVM tools must be available inside the build environment, even when Rust tools are shared with the host.
- **Old title after renaming the Cargo package:** build scripts must package the current binary. They read the package name and build it explicitly; do not manually copy an old executable left under `target`.

## Publishing releases

All outputs stay in `dist/linux` or `dist/windows`. Upload these generated files together, retaining their names:

```text
beat-saber-mapping-extractor-v<version>-linux-<arch>.AppImage
beat-saber-mapping-extractor-v<version>-linux-<arch>.AppImage.zsync
beat-saber-mapping-extractor-v<version>-linux-<arch>.AppImage.sha256
beat-saber-mapping-extractor-v<version>-windows-<arch>.exe
beat-saber-mapping-extractor-v<version>-windows-<arch>.exe.sha256
```

Tags must match the Cargo package version, optionally prefixed with `v`. For pre-releases, use a version such as `1.1.0-rc.1` and mark the release accordingly. AppImage updates remain on the latest stable channel. Build scripts do not create or publish releases.
