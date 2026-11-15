use crate::settings::config_dir;
use std::{
    fs,
    io::{self, Read, Write},
    time::Duration,
};

pub const USERNAME: &str = "TheDoctorTTV";
pub const PROFILE_URL: &str = "https://github.com/TheDoctorTTV";
const MAX_AVATAR_BYTES: u64 = 4 * 1024 * 1024;

pub fn cached() -> Option<Vec<u8>> {
    let path = config_dir().ok()?.join("github-avatar");
    let mut file = fs::File::open(path).ok()?.take(MAX_AVATAR_BYTES + 1);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MAX_AVATAR_BYTES).then_some(bytes)
}

pub fn decode(bytes: &[u8]) -> Result<eframe::egui::ColorImage, String> {
    let mut reader = image::ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(1024);
    limits.max_image_height = Some(1024);
    limits.max_alloc = Some(16 * 1024 * 1024);
    reader.limits(limits);
    let rgba = reader.decode().map_err(|err| err.to_string())?.into_rgba8();
    Ok(eframe::egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        rgba.as_raw(),
    ))
}

pub fn fetch() -> Result<Vec<u8>, String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!(
            "BeatSaberMappingExtractor/",
            env!("CARGO_PKG_VERSION")
        ))
        .build()
        .map_err(|err| err.to_string())?
        .get(format!("https://github.com/{USERNAME}.png?size=192"))
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?;
    let mut bytes = Vec::new();
    response
        .take(MAX_AVATAR_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| err.to_string())?;
    if bytes.len() as u64 > MAX_AVATAR_BYTES {
        return Err("Avatar response is too large".into());
    }
    decode(&bytes)?;
    // A cache write failure must not prevent showing a freshly fetched avatar.
    if let Ok(directory) = config_dir() {
        let _ = (|| -> io::Result<()> {
            fs::create_dir_all(&directory)?;
            let mut file = tempfile::NamedTempFile::new_in(&directory)?;
            file.write_all(&bytes)?;
            file.persist(directory.join("github-avatar"))
                .map_err(|err| err.error)?;
            Ok(())
        })();
    }
    Ok(bytes)
}
