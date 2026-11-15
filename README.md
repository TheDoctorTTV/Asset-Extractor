# Beat Saber Mapping Extractor

A Rust desktop application for Linux and Windows. Python, UnityPy, and PyInstaller are no longer required.

## Features

- Convert gzip-compressed `.gz.asset` files into `.dat` files.
- Save a Unity asset bundle as `raw_asset_bundle.unity3d` for use in AssetStudio.
- Native file/folder selection, progress, activity logs, cancellation, and opening the output folder.
- Work runs on a background thread. Page selections and paths stay intact when switching pages.
- Failed or cancelled writes discard temporary files, preserving existing destination files. Successful writes replace files with the same name.

The extractor copies the original bundle unchanged, matching the previous combined application's behavior. It does not unpack Unity objects, decode textures, or export TextAssets itself. Use [AssetStudio](https://github.com/Perfare/AssetStudio) for that step.

## Build on Linux

Install current stable [Rust and Cargo](https://rustup.rs/), a C/C++ toolchain, and pkg-config. For example:

```bash
# Debian / Ubuntu
sudo apt install build-essential pkg-config libx11-dev libxi-dev libxcursor-dev libxrandr-dev libxinerama-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev

# Fedora
sudo dnf install gcc gcc-c++ make pkgconf-pkg-config libX11-devel libXi-devel libXcursor-devel libXrandr-devel libXinerama-devel libxkbcommon-devel wayland-devel mesa-libGL-devel

# Arch
sudo pacman -S --needed base-devel pkgconf libx11 libxi libxcursor libxrandr libxinerama libxkbcommon wayland mesa
```

Run from any working directory:

```bash
bash /path/to/Beat-Saber-Mapping-Extractor/build_linux.sh
```

Output: `dist/linux/beat-saber-mapping-extractor`. Launch it in a graphical desktop session. Both X11 and Wayland are enabled. Native file dialogs use `xdg-desktop-portal` with a desktop backend such as KDE or GTK; install these if your desktop does not already supply them. The binary uses system graphics/display libraries and is not an AppImage or a fully static build.

## Build on Windows

Install current stable Rust using rustup with its default MSVC toolchain. Install **Visual Studio Build Tools**, selecting **Desktop development with C++** and a Windows SDK, then reopen the terminal.

Double-click `build_windows.bat`, or run:

```powershell
.\build_windows.ps1
```

If PowerShell's script execution policy blocks that command, run `build_windows.bat`, which applies a process-local policy override.

Output: `dist/windows/beat-saber-mapping-extractor.exe`. The existing icon is embedded for the application window. The app has no console window on Windows. Binaries are unsigned.

Each script builds a release binary for the machine running it and copies the executable and README into `dist`. Build Linux on Linux and Windows on Windows; these scripts do not configure cross-compilation. They do not run tests or install system dependencies. Cargo downloads dependencies on the first build and generates `Cargo.lock`; keep that lockfile in version control after your first successful build.

## Usage

1. Open **Extractor**, select your Unity bundle and an output folder, then select **Start Extraction**. The default output is `extracted_assets` beside the executable.
2. Open the saved `raw_asset_bundle.unity3d` in AssetStudio and export its assets.
3. Open **Converter**, select AssetStudio's exported `TextAsset` folder and a destination folder, then select **Start Conversion**.
4. Use **Open Output Folder** to see the result. Files are processed directly inside the selected input folder, without recursion. Only names ending in `.gz.asset` are converted.

Cancel stops the active operation between chunks. Completed files are retained; the current incomplete file is discarded. Closing the application also cancels its worker; a forced termination may leave a temporary file. The activity view retains the latest 2,000 lines. Status messages report individual failures and final counts.

## Source layout

- `src/main.rs`: desktop entry point and embedded window icon.
- `src/app.rs`: GUI and background worker coordination.
- `src/jobs.rs`: streaming gzip conversion and raw bundle copying.
- `build_linux.sh`, `build_windows.ps1`, `build_windows.bat`: native release builds.

## Disclaimer

This tool is provided for educational purposes. Respect the rights of content creators and applicable terms of service.
