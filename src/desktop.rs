//! Register the desktop identity used by Wayland to resolve the window icon.
use std::os::unix::fs::PermissionsExt;
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

pub const APP_ID: &str = "io.github.TheDoctorTTV.AssetExtractor";

pub fn register() -> io::Result<()> {
    let dirs = directories::BaseDirs::new()
        .ok_or_else(|| io::Error::other("Could not locate desktop data directory"))?;
    let executable = std::env::var_os("APPIMAGE")
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or(std::env::current_exe()?)
        .canonicalize()?;
    let executable = executable
        .to_str()
        .ok_or_else(|| io::Error::other("Desktop executable path is not UTF-8"))?;
    let mut command = String::from("\"");
    for character in executable.chars() {
        match character {
            '\n' | '\r' | '\t' => {
                return Err(io::Error::other("Unsupported desktop executable path"))
            }
            '\\' => command.push_str(r"\\\\"),
            '"' => command.push_str(r#"\\""#),
            '$' => command.push_str(r"\\$"),
            '`' => command.push_str(r"\\`"),
            '%' => command.push_str("%%"),
            _ => command.push(character),
        }
    }
    command.push('"');
    let data = dirs.data_dir();
    let icon_path = data.join(format!("icons/hicolor/256x256/apps/{APP_ID}.png"));
    let icon_changed = write_if_changed(
        &icon_path,
        include_bytes!("../assets/io.github.TheDoctorTTV.AssetExtractor.png"),
    )?;
    // This identity entry lets the compositor match app_id even when launched
    // directly. Gear Lever can keep its own visible launcher without duplication.
    // Resolve the PNG directly so an existing icon-theme cache cannot hide it.
    let icon_value = icon_path
        .to_str()
        .ok_or_else(|| io::Error::other("Desktop icon path is not UTF-8"))?
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Asset Extractor\nExec={command}\nIcon={icon_value}\nTerminal=false\nNoDisplay=true\nCategories=Utility;\nStartupWMClass={APP_ID}\n"
    );
    let desktop_changed = write_if_changed(
        &data.join(format!("applications/{APP_ID}.desktop")),
        entry.as_bytes(),
    )?;
    if icon_changed || desktop_changed {
        let _ = std::process::Command::new("gtk-update-icon-cache")
            .args(["-f", "-t"])
            .arg(data.join("icons/hicolor"))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        // Refresh KDE's application lookup before creating the Wayland window.
        // Other desktops can discover the entry through their directory watchers.
        let _ = std::process::Command::new("kbuildsycoca6")
            .arg("--noincremental")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
    Ok(())
}

fn write_if_changed(path: &Path, contents: &[u8]) -> io::Result<bool> {
    if fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(false);
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing desktop data directory"))?;
    fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(contents)?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o644))?;
    file.persist(path).map_err(|err| err.error)?;
    Ok(true)
}
