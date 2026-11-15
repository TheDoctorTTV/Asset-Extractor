# Beat Saber Mapping Extractor

A Rust desktop application for Linux and Windows. Python, UnityPy, and PyInstaller are no longer required.

## Extract in one step

Open **Home**, choose a bundle (including extensionless Beat Saber bundles such as `lfg`), and select **Extract**. Choose a destination when prompted. The app unpacks the bundle, exports its objects, and decompresses gzip TextAssets to `.dat` files automatically. AssetStudio and Python are not required.

Each extraction creates a folder named after the bundle inside the chosen destination. Repeated extractions use `lfg-2`, `lfg-3`, and so on. Existing extractions are preserved. Filenames are made portable, and collisions receive numbered suffixes.

- **Home:** select a bundle, extract, cancel, and open the result.
- **Console:** extraction stages, exported filenames, warnings, and a Copy log button. The view retains the latest 2,000 lines.
- **Settings:** enable a predefined output folder, browse for it, and select **Save settings**. Leave the option off to choose a destination each time.
- **About:** TheDoctorTTV’s GitHub profile and avatar. The avatar refreshes when About first opens in each app session, then on later visits after an hour, or with **Refresh avatar**. A cached copy is used offline. Only avatar refreshes need internet access.

Settings and the cached avatar are stored in the current user’s application configuration directory, using `directories::ProjectDirs` with organization `TheDoctorTTV` and application `BeatSaberMappingExtractor`.

## Export support and limits

Native extraction uses [unity-rs-core](https://github.com/seiunx-dev/unity-rs), pinned to version 0.5.0. It reads Unity containers and serialized objects directly. TextAssets retain their bytes; gzip payloads are detected by their signature and expanded into `.dat`. Supported textures export as PNG and supported audio as WAV. Other supported objects use the backend’s automatic export format. Unsupported objects are retained as raw Unity object bytes (`.bin`) where possible, and reported as warnings. Unrecognized audio codecs retain their original audio container. Raw fallback files are not decoded media.

The provided sample’s header identifies UnityFS format 8, Unity 6000.3.19f1. Reading that header does not establish that every object in the sample exports successfully. This change has not been built or runtime tested.

The output contains extracted assets; it does not reconstruct a complete custom Beat Saber song package, synthesize `Info.dat`, or rewrite map/audio references.

Extraction runs in a worker thread and stages its files before saving a new result folder. Cancellation is checked between decoder stages and during file conversion. A backend decoding stage must finish before cancellation takes effect. Cancelled or failed staging is discarded. The final save moves completed files into a reserved new folder; ordinary save errors attempt to remove that folder. Forced termination or power loss can leave staging or partial output folders. Closing the app requests cancellation but does not wait for the worker to finish.

Default limits include 512 MiB per prepared file and 16 GiB total prepared output, with additional backend limits on parsing, decoding, and object counts. Some large or unsupported assets may produce warnings. Review Console when a result reports warnings.

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

Each script builds a release binary for the machine running it and copies the executable and README into `dist`. Build Linux on Linux and Windows on Windows; these scripts do not configure cross-compilation. They do not run tests or install system dependencies. Cargo downloads dependencies as needed and updates `Cargo.lock` when dependencies change. Keep that lockfile in version control. Use current stable Rust; the extraction backend requires Rust 1.88 or newer and some dependencies may require a newer compiler.

## Source layout

- `src/main.rs`: desktop entry point and embedded window icon.
- `src/app.rs`: Home, Console, Settings, About, and worker coordination.
- `src/jobs.rs`: native Unity export, automatic gzip conversion, and staged saving.
- `src/settings.rs`: persistent output-folder preference.
- `src/avatar.rs`: GitHub avatar fetching, image decoding, and offline caching.
- `build_linux.sh`, `build_windows.ps1`, `build_windows.bat`: native release builds.

## Credits

Unity parsing and export: [unity-rs-core](https://github.com/seiunx-dev/unity-rs) (MIT). Desktop UI: [egui/eframe](https://github.com/emilk/egui) (MIT/Apache-2.0). See the dependencies and their transitive dependencies for their respective licenses.

## Disclaimer

This tool is provided for educational purposes. Respect the rights of content creators and applicable terms of service.
