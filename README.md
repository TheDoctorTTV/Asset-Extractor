# Asset Extractor

A Rust desktop app for extracting Unity bundles on Linux and Windows, with automatic Beat Saber map metadata generation.

## Get started

Download the appropriate AppImage or Windows executable from [GitHub Releases](https://github.com/TheDoctorTTV/Beat-Saber-Mapping-Extractor/releases).

- **Linux:** make the AppImage executable and open it. Integrate it with Gear Lever for a menu shortcut.
- **Windows:** open the `.exe`. Builds are unsigned.

In **Home**, select a bundle, choose **Extract**, and select an output destination. Extensionless bundles are supported. Each extraction creates its own folder; existing folders prompt you to choose a new name, overwrite, or cancel. Overwrite replaces the entire previous folder.

The app exports supported assets, expands gzip TextAssets into `.dat` files, and generates a v4.0.1 `Info.dat`. Review **Console** for warnings before using the result as a song package. Unsupported assets may be preserved as raw files rather than playable media.

For built-in Beat Saber songs, open the original bundle under `Beat Saber_Data/StreamingAssets/BeatmapLevelsData`. Keep the sibling `StreamingAssets/aa` folder available so the app can resolve environment settings.

**Settings** offers saved themes and a default output folder. **About** provides release checks and executable updates. AppImages update through Gear Lever or manual replacement; the built-in updater is hidden when Gear Lever registration is detected at startup.

## Build

Install current stable [Rust](https://rustup.rs/) and the platform prerequisites in [docs.md](docs.md#build-prerequisites). Run from the repository:

| Build | Command | Output |
| --- | --- | --- |
| Linux AppImage | `./build_linux.sh` | `dist/linux` |
| Windows on Windows | `build_windows.bat` or `./build_windows.ps1` | `dist/windows` |
| Windows from Linux | `./build_windows.sh` | `dist/windows` |
| Both from Linux | `./build_all.sh` | Both platform folders |

Windows cross-builds default to x86_64. Pass `aarch64` to `build_windows.sh` or `build_all.sh` for ARM64 Windows. Linux builds for the current Rust host target. The combined script stops if either build fails.

Linux outputs an AppImage, `.zsync`, and `.sha256`; Windows outputs one versioned `.exe` and its `.sha256` for supported release targets. Build scripts prepare files without publishing releases or running tests.

The Cargo package is `asset-extractor`; release filenames retain the `beat-saber-mapping-extractor` prefix for updater compatibility.

## Documentation

See [docs.md](docs.md) for export behavior, metadata fallbacks, output recovery, updater details, dependencies, troubleshooting, and publishing instructions.

## Credits

Built with [unity-rs-core](https://github.com/seiunx-dev/unity-rs), [egui/eframe](https://github.com/emilk/egui), and [self-replace](https://github.com/mitsuhiko/self-replace). See their licenses and those of their dependencies.

For educational use. Respect content creators’ rights and applicable terms of service.
