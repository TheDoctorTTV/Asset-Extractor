use eframe::egui;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const REPO: &str = "TheDoctorTTV/Asset-Extractor";
const MAX_DOWNLOAD: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    #[default]
    Stable,
    Prerelease,
}
impl Channel {
    fn label(self) -> &'static str {
        match self {
            Self::Stable => "Releases",
            Self::Prerelease => "Pre-releases",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckInterval {
    StartupOnly,
    #[default]
    Hourly,
    Daily,
    TwoDays,
    Weekly,
}
impl CheckInterval {
    fn seconds(self) -> u64 {
        match self {
            Self::StartupOnly => 0,
            Self::Hourly => 3600,
            Self::Daily => 86400,
            Self::TwoDays => 172800,
            Self::Weekly => 604800,
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::StartupOnly => "Startup only",
            Self::Hourly => "Every hour",
            Self::Daily => "Every 24 hours",
            Self::TwoDays => "Every 48 hours",
            Self::Weekly => "Every week",
        }
    }
}

#[derive(Clone, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}
#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
struct Release {
    tag: String,
    version: Version,
    binary: Option<Asset>,
    checksum: Option<Asset>,
}

enum Event {
    Checked(Result<Vec<Release>, String>),
    Progress(f32, String),
    Installed(PathBuf),
    Failed(String),
}

// Gear Lever records installed AppImages in gearlever.conf. Our AppImages embed
// gh-releases-zsync metadata, which Gear Lever uses even without a saved manager.
// Match the full running AppImage path, rather than a folder or the presence of
// Gear Lever itself, so unrelated installations retain their built-in updater.
#[cfg(target_os = "linux")]
fn gear_lever_manages_updates() -> bool {
    let Some(appimage) = std::env::var_os("APPIMAGE")
        .filter(|path| !path.is_empty())
        .and_then(|path| std::fs::canonicalize(path).ok())
    else {
        return false;
    };
    let Some(dirs) = directories::BaseDirs::new() else {
        return false;
    };
    let configs = [
        dirs.config_dir().join("gearlever.conf"),
        dirs.home_dir()
            .join(".var/app/it.mijorus.gearlever/config/gearlever.conf"),
    ];
    configs.iter().any(|config| {
        let Ok(contents) = std::fs::read_to_string(config) else {
            return false;
        };
        let mut app_section = false;
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(section) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                app_section = section.strip_prefix("app.").is_some_and(|hash| {
                    hash.len() == 32 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                });
                continue;
            }
            if app_section {
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim().eq_ignore_ascii_case("file_path")
                        && std::fs::canonicalize(value.trim()).is_ok_and(|path| path == appimage)
                    {
                        return true;
                    }
                }
            }
        }
        false
    })
}

#[cfg(not(target_os = "linux"))]
fn gear_lever_manages_updates() -> bool {
    false
}

pub struct Updater {
    enabled: bool,
    pub channel: Channel,
    pub interval: CheckInterval,
    releases: Vec<Release>,
    selected: usize,
    receiver: Option<Receiver<Event>>,
    pub installing: bool,
    installed: bool,
    pub status: String,
    progress: f32,
    checked: bool,
    startup: bool,
    next_check: Instant,
    cancel: Arc<AtomicBool>,
}

impl Updater {
    pub fn new(channel: Channel, interval: CheckInterval) -> Self {
        Self {
            enabled: !gear_lever_manages_updates(),
            channel,
            interval,
            releases: Vec::new(),
            selected: 0,
            receiver: None,
            installing: false,
            installed: false,
            status: "Not checked".into(),
            progress: 0.0,
            checked: false,
            startup: true,
            next_check: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn update_available(&self) -> bool {
        if !self.enabled {
            return false;
        }
        self.releases
            .iter()
            .find(|r| r.binary.is_some())
            .is_some_and(|release| release.version > current_version())
    }
    pub fn check(&mut self) {
        if !self.enabled || self.receiver.is_some() || self.installed {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let channel = self.channel;
        match thread::Builder::new()
            .name("update-check".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(|| fetch_releases(channel))
                    .unwrap_or_else(|_| Err("Update check stopped unexpectedly".into()));
                let _ = tx.send(Event::Checked(result));
            }) {
            Ok(_) => {
                self.receiver = Some(rx);
                self.status = "Checking for updates...".into();
                self.progress = 0.0;
            }
            Err(err) => self.status = format!("Could not check updates: {err}"),
        }
        self.startup = false;
        self.next_check = Instant::now() + Duration::from_secs(self.interval.seconds());
    }
    pub fn tick(&mut self, ctx: &egui::Context) -> Option<PathBuf> {
        if !self.enabled {
            return None;
        }
        for _ in 0..100 {
            let event = match &self.receiver {
                Some(rx) => rx.try_recv(),
                None => break,
            };
            match event {
                Ok(Event::Checked(result)) => {
                    self.receiver = None;
                    match result {
                        Ok(releases) => {
                            let previous = self.releases.get(self.selected).map(|r| r.tag.clone());
                            self.selected = previous
                                .and_then(|tag| releases.iter().position(|r| r.tag == tag))
                                .unwrap_or_else(|| {
                                    releases
                                        .iter()
                                        .position(|r| r.binary.is_some())
                                        .unwrap_or(0)
                                });
                            self.releases = releases;
                            self.checked = true;
                            self.progress = 1.0;
                            self.status = if self.update_available() {
                                "Update available"
                            } else if self
                                .releases
                                .first()
                                .is_some_and(|r| r.version <= current_version())
                            {
                                "Up to date"
                            } else if self.releases.is_empty() {
                                "No releases available for this channel"
                            } else {
                                "No compatible download available"
                            }
                            .into();
                        }
                        Err(err) => self.status = format!("Update check failed: {err}"),
                    }
                    break;
                }
                Ok(Event::Progress(value, status)) => {
                    self.progress = value;
                    self.status = status;
                }
                Ok(Event::Installed(path)) => {
                    self.receiver = None;
                    self.installing = false;
                    self.installed = true;
                    self.status = "Installed. Restarting...".into();
                    self.progress = 1.0;
                    return Some(path);
                }
                Ok(Event::Failed(err)) => {
                    self.receiver = None;
                    self.installing = false;
                    self.status = format!("Install failed: {err}");
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.receiver = None;
                    self.installing = false;
                    self.status = "Update worker stopped unexpectedly".into();
                    break;
                }
            }
        }
        if !self.installed
            && (self.startup || (self.interval.seconds() != 0 && Instant::now() >= self.next_check))
        {
            self.check();
        }
        if self.receiver.is_some() {
            ctx.request_repaint_after(Duration::from_millis(80));
        } else if !self.installed && self.interval.seconds() != 0 {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
        None
    }
    fn install(&mut self) {
        if !self.enabled || self.receiver.is_some() || self.installed {
            return;
        }
        let Some(release) = self.releases.get(self.selected) else {
            return;
        };
        let Some(binary) = release.binary.clone() else {
            return;
        };
        let checksum = release.checksum.clone();
        let (tx, rx) = mpsc::channel();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        match thread::Builder::new()
            .name("update-install".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    install_binary(&binary, checksum.as_ref(), &tx, &cancel)
                }));
                let event = match result {
                    Ok(Ok(path)) => Event::Installed(path),
                    Ok(Err(err)) => Event::Failed(err.to_string()),
                    Err(_) => Event::Failed("Update installer stopped unexpectedly".into()),
                };
                let _ = tx.send(event);
            }) {
            Ok(_) => {
                self.receiver = Some(rx);
                self.installing = true;
                self.progress = 0.0;
                self.status = "Downloading selected version...".into();
            }
            Err(err) => self.status = format!("Could not start install: {err}"),
        }
    }
    // Returns true when the user changes persisted preferences.
    pub fn show(&mut self, ui: &mut egui::Ui, extraction_busy: bool) -> bool {
        if !self.enabled {
            return false;
        }
        let old_channel = self.channel;
        let old_interval = self.interval;
        let busy = self.receiver.is_some() || self.installed;
        let installable = self
            .releases
            .get(self.selected)
            .is_some_and(|r| r.binary.is_some());
        let install_label = match self
            .releases
            .get(self.selected)
            .map(|r| r.version.cmp(&current_version()))
        {
            Some(std::cmp::Ordering::Less) => "Downgrade",
            Some(std::cmp::Ordering::Equal) => "Reinstall",
            _ => "Update",
        };
        let mut check_clicked = false;
        let mut install_clicked = false;
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("App Updates").size(16.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                install_clicked = ui
                    .add_enabled(
                        !busy && !extraction_busy && installable,
                        egui::Button::new(install_label),
                    )
                    .on_hover_text("Install the selected version and restart the app")
                    .clicked();
                check_clicked = ui.add_enabled(!busy, egui::Button::new("Check")).clicked();
            });
        });
        ui.horizontal(|ui| {
            let color = if self.update_available() {
                ui.visuals().selection.bg_fill
            } else {
                ui.visuals().text_color()
            };
            let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 4.0, color);
            ui.add(egui::Label::new(&self.status).truncate())
                .on_hover_text(&self.status);
        });
        let active = self.receiver.is_some() || self.installing;
        ui.add(
            egui::ProgressBar::new(if active { self.progress } else { 0.0 })
                .desired_height(6.0)
                .animate(active),
        );
        ui.add_space(8.0);
        // Match stream247's labeled fields, wrapping whole fields on small windows.
        let field_width = ((ui.available_width() - 24.0) / 3.0)
            .max(180.0)
            .min(ui.available_width());
        ui.add_enabled_ui(!busy, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(field_width);
                    ui.small("Channel");
                    egui::ComboBox::from_id_salt("update-channel")
                        .width(field_width)
                        .selected_text(self.channel.label())
                        .show_ui(ui, |ui| {
                            for channel in [Channel::Stable, Channel::Prerelease] {
                                ui.selectable_value(&mut self.channel, channel, channel.label());
                            }
                        });
                });
                ui.vertical(|ui| {
                    ui.set_width(field_width);
                    ui.small("Version");
                    ui.add_enabled_ui(old_channel == self.channel, |ui| {
                        let selected = self
                            .releases
                            .get(self.selected)
                            .map(|r| r.tag.trim_start_matches(['v', 'V']))
                            .unwrap_or("Check to load versions");
                        egui::ComboBox::from_id_salt("update-version")
                            .width(field_width)
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for (index, release) in self.releases.iter().enumerate() {
                                    let label = if release.binary.is_some() {
                                        release.version.to_string()
                                    } else {
                                        format!("{} (download unavailable)", release.version)
                                    };
                                    ui.selectable_value(&mut self.selected, index, label);
                                }
                            });
                    });
                });
                ui.vertical(|ui| {
                    ui.set_width(field_width);
                    ui.small("Checks after startup");
                    egui::ComboBox::from_id_salt("update-interval")
                        .width(field_width)
                        .selected_text(self.interval.label())
                        .show_ui(ui, |ui| {
                            for interval in [
                                CheckInterval::StartupOnly,
                                CheckInterval::Hourly,
                                CheckInterval::Daily,
                                CheckInterval::TwoDays,
                                CheckInterval::Weekly,
                            ] {
                                ui.selectable_value(&mut self.interval, interval, interval.label());
                            }
                        });
                });
            });
        });
        let channel_changed = old_channel != self.channel;
        if channel_changed {
            self.releases.clear();
            self.selected = 0;
            self.checked = false;
            self.check();
        } else if check_clicked {
            self.check();
        } else if install_clicked {
            self.install();
        }
        if old_interval != self.interval {
            self.next_check = Instant::now() + Duration::from_secs(self.interval.seconds());
        }
        if extraction_busy {
            ui.small("Finish or cancel extraction before installing an update.");
        }
        channel_changed || old_interval != self.interval
    }
}
impl Drop for Updater {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn current_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("Cargo package version")
}

fn platform_name() -> Option<String> {
    if (cfg!(target_os = "linux") && !cfg!(target_env = "gnu"))
        || (cfg!(target_os = "windows") && !cfg!(target_env = "msvc"))
    {
        return None;
    }
    if !matches!(std::env::consts::ARCH, "x86_64" | "aarch64") {
        return None;
    }
    if !matches!(std::env::consts::OS, "linux" | "windows") {
        return None;
    }
    Some(format!(
        "{}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    ))
}
fn asset_name(version: &Version) -> Option<String> {
    Some(format!(
        "asset-extractor-v{version}-{}{}",
        platform_name()?,
        if cfg!(windows) { ".exe" } else { ".AppImage" }
    ))
}
fn client(timeout: Duration) -> Result<reqwest::blocking::Client, reqwest::Error> {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .user_agent(concat!("AssetExtractor/", env!("CARGO_PKG_VERSION")))
        .build()
}
fn fetch_releases(channel: Channel) -> Result<Vec<Release>, String> {
    let client = client(Duration::from_secs(30)).map_err(|err| err.to_string())?;
    let mut releases = Vec::new();
    for page in 1..=20 {
        let response = client
            .get(format!(
                "https://api.github.com/repos/{REPO}/releases?per_page=100&page={page}"
            ))
            .header("Accept", "application/vnd.github+json")
            .send()
            .map_err(|err| err.to_string())?
            .error_for_status()
            .map_err(|err| err.to_string())?;
        let mut bytes = Vec::new();
        response
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|err| err.to_string())?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err("GitHub response is too large".into());
        }
        let entries: Vec<ApiRelease> =
            serde_json::from_slice(&bytes).map_err(|err| err.to_string())?;
        let done = entries.len() < 100;
        for entry in entries {
            if entry.draft || entry.prerelease != (channel == Channel::Prerelease) {
                continue;
            }
            let Ok(version) = Version::parse(entry.tag_name.trim_start_matches(['v', 'V'])) else {
                continue;
            };
            if version.major < 1 {
                continue;
            } // Python-era releases cannot replace this Rust app.
            let name = asset_name(&version);
            let binary = entry
                .assets
                .iter()
                .find(|a| Some(&a.name) == name.as_ref())
                .cloned();
            let checksum = binary
                .as_ref()
                .and_then(|b| {
                    entry
                        .assets
                        .iter()
                        .find(|a| a.name == format!("{}.sha256", b.name))
                })
                .cloned();
            releases.push(Release {
                tag: entry.tag_name,
                version,
                binary,
                checksum,
            });
        }
        if done {
            releases.sort_by(|a, b| b.version.cmp(&a.version));
            return Ok(releases);
        }
    }
    Err("Too many release pages; check GitHub directly".into())
}
fn checked_url(url: &str) -> io::Result<()> {
    if !url.starts_with(&format!("https://github.com/{REPO}/releases/download/")) {
        return Err(io::Error::other("Unexpected release download URL"));
    }
    Ok(())
}
fn check_cancel(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Update cancelled",
        ))
    } else {
        Ok(())
    }
}
fn install_binary(
    asset: &Asset,
    checksum: Option<&Asset>,
    tx: &Sender<Event>,
    cancel: &AtomicBool,
) -> io::Result<PathBuf> {
    checked_url(&asset.browser_download_url)?;
    if asset.size == 0 || asset.size > MAX_DOWNLOAD {
        return Err(io::Error::other("Unsupported release asset size"));
    }
    let client = client(Duration::from_secs(1800)).map_err(io::Error::other)?;
    let expected = if let Some(digest) = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
    {
        digest.to_owned()
    } else {
        let checksum = checksum
            .ok_or_else(|| io::Error::other("Release has no SHA-256 digest or checksum asset"))?;
        checked_url(&checksum.browser_download_url)?;
        let response = client
            .get(&checksum.browser_download_url)
            .send()
            .map_err(io::Error::other)?
            .error_for_status()
            .map_err(io::Error::other)?;
        let mut body = String::new();
        response.take(4097).read_to_string(&mut body)?;
        if body.len() > 4096 {
            return Err(io::Error::other("Checksum response is too large"));
        }
        body.split_whitespace()
            .next()
            .ok_or_else(|| io::Error::other("Empty release checksum"))?
            .to_owned()
    };
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::other("Invalid SHA-256 checksum"));
    }
    // AppImage current_exe points into the read-only mounted payload.
    // APPIMAGE identifies the outer file that must be replaced and restarted.
    #[cfg(target_os = "linux")]
    let original = match std::env::var_os("APPIMAGE").filter(|path| !path.is_empty()) {
        Some(path) => std::fs::canonicalize(path)?,
        None => std::env::current_exe()?.canonicalize()?,
    };
    #[cfg(not(target_os = "linux"))]
    let original = std::env::current_exe()?.canonicalize()?;
    let staging = tempfile::Builder::new()
        .prefix("asset-extractor-update-")
        .tempdir()?;
    let path = staging.path().join(if cfg!(windows) {
        "update.exe"
    } else {
        "update"
    });
    let mut file = File::create(&path)?;
    let mut response = client
        .get(&asset.browser_download_url)
        .send()
        .map_err(io::Error::other)?
        .error_for_status()
        .map_err(io::Error::other)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    let mut downloaded = 0_u64;
    let mut last = Instant::now();
    loop {
        check_cancel(cancel)?;
        let count = response.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        downloaded += count as u64;
        if downloaded > asset.size {
            return Err(io::Error::other(
                "Download exceeded the published asset size",
            ));
        }
        file.write_all(&buffer[..count])?;
        hash.update(&buffer[..count]);
        if last.elapsed() >= Duration::from_millis(150) {
            tx.send(Event::Progress(
                downloaded as f32 / asset.size as f32 * 0.9,
                format!("Downloading: {downloaded} / {} bytes", asset.size),
            ))
            .map_err(io::Error::other)?;
            last = Instant::now();
        }
    }
    if downloaded != asset.size {
        return Err(io::Error::other("Incomplete update download"));
    }
    if format!("{:x}", hash.finalize()) != expected.to_ascii_lowercase() {
        return Err(io::Error::other(
            "Downloaded update failed SHA-256 verification",
        ));
    }
    file.sync_all()?;
    drop(file);
    validate_binary(&path)?;
    check_cancel(cancel)?;
    tx.send(Event::Progress(
        0.95,
        "Installing verified update...".into(),
    ))
    .map_err(io::Error::other)?;
    // Keep an independent copy until replacement succeeds. In particular, Windows
    // replacement can fail after moving the running executable out of the way.
    let backup = tempfile::Builder::new()
        .prefix(".asset-extractor-update-backup-")
        .suffix(if cfg!(windows) { ".exe" } else { ".AppImage" })
        .tempfile_in(
            original
                .parent()
                .ok_or_else(|| io::Error::other("Executable has no parent directory"))?,
        )?;
    std::fs::copy(&original, backup.path())?;
    backup.as_file().sync_all()?;
    check_cancel(cancel)?;
    #[cfg(target_os = "linux")]
    let replacement = replace_linux_appimage(&path, &original);
    #[cfg(not(target_os = "linux"))]
    let replacement = self_replace::self_replace(&path);
    if let Err(err) = replacement {
        let (_, backup_path) = backup.keep().map_err(|keep| keep.error)?;
        if !original.exists() {
            if let Err(restore) = std::fs::copy(&backup_path, &original) {
                return Err(io::Error::other(format!("Replacement failed: {err}; restore failed: {restore}. Previous executable preserved at {}", backup_path.display())));
            }
        }
        return Err(io::Error::other(format!(
            "Replacement failed: {err}. Previous executable preserved at {}",
            backup_path.display()
        )));
    }
    Ok(original)
}
// Prepare the complete replacement beside the destination, then rename it
// atomically. The mounted old image remains available until its process exits.
#[cfg(target_os = "linux")]
fn replace_linux_appimage(
    source: &std::path::Path,
    destination: &std::path::Path,
) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("AppImage has no parent directory"))?;
    let mut replacement = tempfile::NamedTempFile::new_in(parent)?;
    let mut source = File::open(source)?;
    io::copy(&mut source, replacement.as_file_mut())?;
    let permissions = std::fs::metadata(destination)?.permissions().mode();
    replacement
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(permissions | 0o111))?;
    replacement.as_file().sync_all()?;
    replacement.persist(destination).map_err(|err| err.error)?;
    Ok(())
}

fn validate_binary(path: &std::path::Path) -> io::Result<()> {
    let mut file = File::open(path)?;
    let mut header = [0; 64];
    file.read_exact(&mut header)?;
    if cfg!(target_os = "linux") {
        let machine = u16::from_le_bytes([header[18], header[19]]);
        let expected = if cfg!(target_arch = "aarch64") {
            183
        } else {
            62
        };
        if &header[..4] != b"\x7fELF"
            || header[4] != 2
            || header[5] != 1
            || machine != expected
            || &header[8..11] != b"AI\x02"
        {
            return Err(io::Error::other(
                "Update is not a compatible Linux AppImage",
            ));
        }
    } else if cfg!(target_os = "windows") {
        if &header[..2] != b"MZ" {
            return Err(io::Error::other("Update is not a Windows executable"));
        }
        let offset = u32::from_le_bytes(header[60..64].try_into().unwrap());
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut pe = [0; 6];
        file.read_exact(&mut pe)?;
        let expected = if cfg!(target_arch = "aarch64") {
            0xaa64
        } else {
            0x8664
        };
        if &pe[..4] != b"PE\0\0" || u16::from_le_bytes([pe[4], pe[5]]) != expected {
            return Err(io::Error::other(
                "Update is not a compatible Windows executable",
            ));
        }
    } else {
        return Err(io::Error::other(
            "In-app updates are supported on Linux and Windows",
        ));
    }
    Ok(())
}
