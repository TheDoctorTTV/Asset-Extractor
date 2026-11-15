# Beat Saber Mapping Extractor

A Rust desktop application for Linux and Windows. Python, UnityPy, and PyInstaller are no longer required.

## Extract in one step

Open **Home**, choose a bundle (including extensionless Beat Saber bundles such as `lfg`), and select **Extract**. Choose a destination when prompted. The app unpacks the bundle, exports its objects, decompresses gzip TextAssets to `.dat` files, and generates `Info.dat` automatically. AssetStudio and Python are not required.

Each extraction creates a folder named after the bundle inside the chosen destination. If that folder already exists, a prompt offers **Extract to new folder**, **Overwrite previous extraction**, or **Cancel**. The new-folder option suggests an unused name such as `lfg-2` and lets you edit it. Overwrite replaces the whole previous extraction folder, including any files added to it. Individual asset filenames are made portable, and collisions receive numbered suffixes.

- **Home:** select a bundle, extract, cancel, and open the result.
- **Console:** extraction stages, exported filenames, warnings, and a Copy log button. The view retains the latest 2,000 lines.
- **Settings:** enable a predefined output folder, browse for it, and select **Save settings**. Leave the option off to choose a destination each time.
- **About:** TheDoctorTTV’s GitHub profile and avatar. The avatar refreshes when About first opens in each app session, then on later visits after an hour, directly from [the GitHub avatar URL](https://avatars.githubusercontent.com/u/39390981?s=96&v=4). No avatar is saved to disk. Only avatar refreshes need internet access.

Settings are stored in the current user’s application configuration directory, using `directories::ProjectDirs` with organization `TheDoctorTTV` and application `BeatSaberMappingExtractor`.

## Export support and limits

Native extraction uses [unity-rs-core](https://github.com/seiunx-dev/unity-rs), pinned to version 0.5.0. It reads Unity containers and serialized objects directly. TextAssets retain their bytes; gzip payloads are detected by their signature and expanded into `.dat`. Supported textures export as PNG and supported audio as WAV. Other supported objects use the backend’s automatic export format. Unsupported objects are retained as raw Unity object bytes (`.bin`) where possible, and reported as warnings. Unrecognized audio codecs retain their original audio container. Raw fallback files are not decoded media.

The provided sample’s header identifies UnityFS format 8, Unity 6000.3.19f1. Reading that header does not establish that every object in the sample exports successfully. This change has not been built or runtime tested.

### Generated Info.dat

Every extraction also writes a v4.0.1 `Info.dat` using the exported filenames for song audio, preview, cover art, audio timing data, difficulty beatmaps, and lightshows. It uses a bundled v4 info file’s song metadata, difficulty settings, credits, environments, and color schemes when available. The bundled original is preserved under a numbered name such as `Info-2.dat`.

Without a bundled info file, difficulty and characteristic names are inferred from filenames such as `ExpertPlus.beatmap.dat`, `ExpertPlusOneSaber.dat`, or `NoArrowsHard.dat`. Unqualified names use Standard. Matching difficulty lightshows take priority, followed by a characteristic lightshow, `Common.lightshow.dat`, or a single shared lightshow. Ambiguous or missing references are reported in Console. Unknown difficulties are omitted with a warning.

Song duration and initial BPM can be derived from the sample count, sample rate, and first BPM region in the selected AudioData file. The complete timing payload is preserved, including BPM regions and its checksum, and saved as `AudioData.dat`. The generated `Info.dat` references that exact name. This is required for the official editor, whose working-folder copy only includes the literal filename `AudioData.dat`; bundle names such as `LFG.audio.dat` would otherwise be omitted. When metadata is unavailable, the song title falls back to the bundle name, BPM to 120, duration to 0, environment to `DefaultEnvironment`, credits to empty, and jump settings to 0. Console reports missing critical values and inferred difficulty settings. The example’s song-specific BPM, environment, and filenames are not copied into other songs.

Review warnings before using the result as a song package. This generates the entry point and its file references; it does not restore metadata absent from the bundle or convert unsupported audio/media into playable formats. Schema references: [Info format](https://bsmg.wiki/mapping/map-format/info.html), [Audio timing format](https://bsmg.wiki/mapping/map-format/audio.html).

Extraction runs in a worker thread and stages its files before saving a new result folder. Cancellation is checked between decoder stages and during file conversion. A backend decoding stage must finish before cancellation takes effect. Cancelled or failed staging is discarded. The final save uses the chosen folder name. Overwrite keeps the previous extraction until all new assets have been prepared, then moves it to a temporary backup before saving the replacement. If saving fails, the app attempts to restore the previous folder; recovery failures report the preserved backup path in Console. A successful replacement removes the backup. A new-folder destination that appears during decoding is rejected rather than overwritten. Forced termination or power loss can leave staging, backup, or partial output folders. Closing the app requests cancellation but does not wait for the worker to finish.

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

### Linux icon and launcher

The Linux build also registers a launcher and the app icon for your current user under `${XDG_DATA_HOME:-$HOME/.local/share}/applications` and `icons/hicolor`. Run the build as your normal user. The desktop entry matches the window’s Wayland application ID; X11 also retains the embedded window icon. The application menu entry is **Beat Saber Mapping Extractor**.

The launcher points at the executable in `dist/linux`. After rebuilding, close and reopen the app to pick up the new application ID and icon. If you previously pinned the generic icon, unpin it and pin the new launcher. If you move or distribute the `dist/linux` folder, run its `install_desktop.sh` to register the new location for that user. Keep the PNG and installer beside the executable.

For packaging or CI without desktop registration, use `INSTALL_DESKTOP_ENTRY=0 ./build_linux.sh`. Linux executable files do not carry a file-manager icon in the same way Windows executables do; use the installed desktop launcher for that icon. See the [desktop entry icon specification](https://specifications.freedesktop.org/desktop-entry/latest/recognized-keys.html).

## Build on Windows

Install current stable Rust using rustup with its default MSVC toolchain. Install **Visual Studio Build Tools**, selecting **Desktop development with C++** and a Windows SDK, then reopen the terminal.

Double-click `build_windows.bat`, or run:

```powershell
.\build_windows.ps1
```

If PowerShell's script execution policy blocks that command, run `build_windows.bat`, which applies a process-local policy override.

Output: `dist/windows/beat-saber-mapping-extractor.exe`. The existing icon is embedded for the application window. The app has no console window on Windows. Binaries are unsigned.

Each script builds a release binary for the machine running it and copies the executable and README into `dist`. Linux also includes the icon and desktop registration script described above. Build Linux on Linux and Windows on Windows; these scripts do not configure cross-compilation. They do not run tests or install system dependencies. Cargo downloads dependencies as needed and updates `Cargo.lock` when dependencies change. Keep that lockfile in version control. Use current stable Rust; the extraction backend requires Rust 1.88 or newer and some dependencies may require a newer compiler.

## Source layout

- `src/main.rs`: desktop entry point and embedded window icon.
- `src/app.rs`: Home, Console, Settings, About, and worker coordination.
- `src/jobs.rs`: native Unity export, automatic gzip conversion, and staged saving.
- `src/map_info.rs`: v4.0.1 Info.dat generation from exported assets and metadata.
- `src/settings.rs`: persistent output-folder preference.
- `src/avatar.rs`: GitHub avatar fetching and in-memory image decoding.
- `build_linux.sh`, `build_windows.ps1`, `build_windows.bat`: native release builds.

## Credits

Unity parsing and export: [unity-rs-core](https://github.com/seiunx-dev/unity-rs) (MIT). Desktop UI: [egui/eframe](https://github.com/emilk/egui) (MIT/Apache-2.0). See the dependencies and their transitive dependencies for their respective licenses.

## Disclaimer

This tool is provided for educational purposes. Respect the rights of content creators and applicable terms of service.
