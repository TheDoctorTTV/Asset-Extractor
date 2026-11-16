use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme: crate::theme::Theme,
    pub use_default_folder: bool,
    pub default_folder: String,
    pub close_to_tray: bool,
    pub update_channel: crate::updater::Channel,
    pub update_check_interval: crate::updater::CheckInterval,
    pub automatic_updates: bool,
}

pub fn config_dir() -> io::Result<PathBuf> {
    ProjectDirs::from("", "TheDoctorTTV", "AssetExtractor")
        .map(|dirs| dirs.config_dir().to_owned())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Could not locate your settings folder",
            )
        })
}

impl Settings {
    pub fn load() -> io::Result<Self> {
        let path = config_dir()?.join("settings.json");
        match fs::read(path) {
            Ok(data) => serde_json::from_slice(&data).map_err(io::Error::other),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(err) => Err(err),
        }
    }

    pub fn save(&self) -> io::Result<()> {
        if self.use_default_folder && self.default_folder.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Choose a default output folder first",
            ));
        }
        let directory = config_dir()?;
        fs::create_dir_all(&directory)?;
        let mut file = tempfile::NamedTempFile::new_in(&directory)?;
        serde_json::to_writer_pretty(&mut file, self).map_err(io::Error::other)?;
        file.write_all(b"\n")?;
        file.as_file_mut().sync_all()?;
        file.persist(directory.join("settings.json"))
            .map_err(|err| err.error)?;
        Ok(())
    }
}
