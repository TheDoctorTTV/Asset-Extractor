use crate::{
    avatar,
    jobs::{self, Event},
    settings::Settings,
};
use eframe::egui::{self, Color32, RichText};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const BACKGROUND: Color32 = Color32::from_rgb(73, 62, 77);
const ACCENT: Color32 = Color32::from_rgb(174, 36, 211);
const FIELD: Color32 = Color32::from_rgb(220, 220, 220);
const MUTED: Color32 = Color32::from_rgb(204, 193, 207);

#[derive(Clone, Copy, Default, PartialEq)]
enum Page {
    #[default]
    Home,
    Console,
    Settings,
    About,
}

pub struct ExtractorApp {
    page: Page,
    bundle: String,
    settings: Settings,
    draft: Settings,
    settings_status: String,
    receiver: Option<Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    progress: f32,
    status: String,
    logs: VecDeque<String>,
    last_output: Option<PathBuf>,
    avatar: Option<egui::TextureHandle>,
    avatar_receiver: Option<Receiver<Result<Vec<u8>, String>>>,
    avatar_requested: Option<Instant>,
    avatar_status: String,
}

impl ExtractorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = BACKGROUND;
        style.visuals.window_fill = BACKGROUND;
        style.visuals.extreme_bg_color = Color32::from_rgb(60, 50, 64);
        style.visuals.override_text_color = Some(Color32::WHITE);
        style.visuals.selection.bg_fill = ACCENT;
        style.visuals.hyperlink_color = Color32::from_rgb(231, 179, 247);
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.animation_time = 0.15;
        ctx.set_style(style);
        let (settings, settings_status) = match Settings::load() {
            Ok(settings) => (settings, String::new()),
            Err(err) => (
                Settings::default(),
                format!("Could not load settings: {err}"),
            ),
        };
        let avatar = avatar::cached()
            .and_then(|bytes| avatar::decode(&bytes).ok())
            .map(|image| ctx.load_texture("github-avatar", image, egui::TextureOptions::LINEAR));
        let mut logs = VecDeque::new();
        if !settings_status.is_empty() {
            logs.push_back(settings_status.clone());
        }
        Self {
            page: Page::Home,
            bundle: String::new(),
            draft: settings.clone(),
            settings,
            settings_status,
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: 0.0,
            status: "Select a bundle to extract its assets.".into(),
            logs,
            last_output: None,
            avatar,
            avatar_receiver: None,
            avatar_requested: None,
            avatar_status: String::new(),
        }
    }

    fn log(&mut self, line: String) {
        if self.logs.len() >= 2000 {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        // Limit each frame so a long console stream cannot starve rendering.
        for _ in 0..200 {
            let event = match &self.receiver {
                Some(rx) => rx.try_recv(),
                None => break,
            };
            match event {
                Ok(Event::Log(line)) => self.log(line),
                Ok(Event::Progress(value, status)) => {
                    self.progress = value;
                    if !self.cancel.load(Ordering::Relaxed) {
                        self.status = status;
                    }
                }
                Ok(Event::Finished { status, output }) => {
                    self.log(status.clone());
                    self.status = status;
                    if output.is_some() {
                        self.last_output = output;
                    }
                    self.receiver = None;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.status = "The extraction worker stopped unexpectedly.".into();
                    self.log(self.status.clone());
                    self.receiver = None;
                    break;
                }
            }
        }
        if let Some(rx) = &self.avatar_receiver {
            match rx.try_recv() {
                Ok(result) => {
                    self.avatar_receiver = None;
                    match result.and_then(|bytes| avatar::decode(&bytes)) {
                        Ok(image) => {
                            self.avatar = Some(ctx.load_texture(
                                "github-avatar",
                                image,
                                egui::TextureOptions::LINEAR,
                            ));
                            self.avatar_status = "Avatar updated from GitHub.".into();
                        }
                        Err(err) => {
                            self.avatar_status = if self.avatar.is_some() {
                                "Offline — showing saved avatar."
                            } else {
                                "Avatar unavailable. Use Refresh to try again."
                            }
                            .into();
                            self.log(format!("GitHub avatar: {err}"));
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.avatar_receiver = None;
                    self.avatar_status = "Avatar refresh stopped. Use Refresh to try again.".into();
                }
                Err(TryRecvError::Empty) => {}
            }
        }
    }

    fn refresh_avatar(&mut self) {
        if self.avatar_receiver.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        match thread::Builder::new()
            .name("github-avatar".into())
            .spawn(move || {
                let _ = tx.send(avatar::fetch());
            }) {
            Ok(_) => {
                self.avatar_receiver = Some(rx);
                self.avatar_requested = Some(Instant::now());
                self.avatar_status = "Updating avatar...".into();
            }
            Err(err) => self.avatar_status = format!("Could not refresh avatar: {err}"),
        }
    }

    fn start(&mut self) {
        let input = PathBuf::from(self.bundle.trim());
        if self.bundle.trim().is_empty() || !input.is_file() {
            self.status = "Choose an existing bundle file first.".into();
            return;
        }
        let output = if self.settings.use_default_folder {
            PathBuf::from(self.settings.default_folder.trim())
        } else {
            let Some(folder) = rfd::FileDialog::new()
                .set_title("Save extracted assets in...")
                .pick_folder()
            else {
                self.status = "Extraction cancelled before starting.".into();
                return;
            };
            folder
        };
        if output.as_os_str().is_empty() {
            self.status = "Choose a default output folder in Settings.".into();
            return;
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        let (tx, rx) = mpsc::channel();
        let input_label = input.display().to_string();
        let output_label = output.display().to_string();
        match thread::Builder::new()
            .name("asset-extractor".into())
            .spawn(move || jobs::run(input, output, tx, cancel))
        {
            Ok(_) => {
                self.receiver = Some(rx);
                self.progress = 0.0;
                self.status = "Starting extraction...".into();
                self.log(format!("Bundle: {input_label}"));
                self.log(format!("Destination: {output_label}"));
            }
            Err(err) => self.status = format!("Could not start extraction: {err}"),
        }
    }

    fn open_output(&mut self) {
        if let Some(output) = &self.last_output {
            if let Err(err) = open::that_detached(output) {
                self.status = format!("Could not open output folder: {err}");
            }
        }
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let title_width = 420.0;
            ui.add_space(((ui.available_width() - title_width) / 2.0).max(0.0));
            ui.label(RichText::new("Asset Extractor").size(48.0));
            ui.label(
                RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                    .size(11.0)
                    .color(MUTED),
            );
        });
        ui.add_space(22.0);
        ui.horizontal(|ui| {
            let width = 135.0;
            ui.add_space(((ui.available_width() - (width * 4.0 + 30.0)) / 2.0).max(0.0));
            for (page, label) in [
                (Page::Home, "Home"),
                (Page::Console, "Console"),
                (Page::Settings, "Settings"),
                (Page::About, "About"),
            ] {
                let active = self.page == page;
                let button =
                    egui::Button::new(RichText::new(label).size(23.0).color(Color32::WHITE))
                        .fill(if active {
                            ACCENT
                        } else {
                            Color32::from_rgb(145, 37, 173)
                        })
                        .rounding(22.0)
                        .stroke(if active {
                            egui::Stroke::new(1.0, Color32::from_rgb(226, 153, 244))
                        } else {
                            egui::Stroke::NONE
                        });
                if ui.add_sized([width, 38.0], button).clicked() {
                    self.page = page;
                }
            }
        });
        ui.add_space(2.0);
        ui.separator();
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        let busy = self.receiver.is_some();
        ui.add_space((ui.available_height() * 0.24).clamp(30.0, 155.0));
        ui.vertical_centered(|ui| {
            ui.set_max_width(580.0);
            ui.add_enabled_ui(!busy, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [
                            410.0_f32.min((ui.available_width() - 150.0).max(150.0)),
                            38.0,
                        ],
                        egui::TextEdit::singleline(&mut self.bundle)
                            .hint_text("Select a Unity bundle")
                            .background_color(FIELD)
                            .text_color(Color32::from_rgb(45, 38, 49))
                            .margin(egui::vec2(14.0, 9.0)),
                    );
                    if action(ui, "Browse", [135.0, 38.0]).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Select a Unity asset bundle")
                            .pick_file()
                        {
                            self.bundle = path.to_string_lossy().into_owned();
                        }
                    }
                });
                ui.add_space(5.0);
                if action(ui, "Extract", [145.0, 38.0]).clicked() {
                    self.start();
                }
            });
            ui.add_space(16.0);
            if busy {
                ui.add(
                    egui::ProgressBar::new(self.progress)
                        .fill(ACCENT)
                        .animate(true)
                        .desired_width(400.0),
                );
                ui.spinner();
            }
            ui.label(RichText::new(&self.status).color(MUTED));
            if busy {
                if ui
                    .add_enabled(
                        !self.cancel.load(Ordering::Relaxed),
                        egui::Button::new("Cancel"),
                    )
                    .clicked()
                {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.status = "Cancelling after the current decoder stage...".into();
                    self.log(
                        "Cancellation requested. Waiting for the current decoder stage.".into(),
                    );
                }
            } else if self.last_output.is_some() {
                if ui.button("Open output folder").clicked() {
                    self.open_output();
                }
            }
            ui.add_space(8.0);
            if self.settings.use_default_folder {
                ui.small(format!("Save to: {}", self.settings.default_folder));
            } else {
                ui.small("You’ll choose where to save when you extract.");
            }
        });
    }

    fn console(&mut self, ui: &mut egui::Ui) {
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.heading("Console");
            if ui.button("Copy log").clicked() {
                ui.output_mut(|output| {
                    output.copied_text = self.logs.iter().cloned().collect::<Vec<_>>().join("\n")
                });
            }
            if ui
                .add_enabled(self.receiver.is_none(), egui::Button::new("Clear"))
                .clicked()
            {
                self.logs.clear();
            }
        });
        ui.label(&self.status);
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.logs.is_empty() {
                    ui.label(RichText::new("Extraction activity will appear here.").color(MUTED));
                }
                for line in &self.logs {
                    ui.monospace(line);
                }
            });
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        ui.add_space(55.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(580.0);
            ui.heading("Save location");
            ui.add_space(12.0);
            ui.checkbox(
                &mut self.draft.use_default_folder,
                "Always use this output folder",
            );
            ui.add_space(8.0);
            ui.add_enabled_ui(self.draft.use_default_folder, |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized(
                        [
                            400.0_f32.min((ui.available_width() - 100.0).max(150.0)),
                            32.0,
                        ],
                        egui::TextEdit::singleline(&mut self.draft.default_folder)
                            .hint_text("Output folder"),
                    );
                    if ui.button("Browse...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Default extraction folder")
                            .pick_folder()
                        {
                            self.draft.default_folder = path.to_string_lossy().into_owned();
                        }
                    }
                });
            });
            ui.label(
                RichText::new("Leave this off to choose a destination for each extraction.")
                    .color(MUTED),
            );
            ui.small("Each bundle is saved in its own folder. Existing extractions are kept.");
            ui.add_space(14.0);
            if action(ui, "Save settings", [170.0, 38.0]).clicked() {
                match self.draft.save() {
                    Ok(()) => {
                        self.settings = self.draft.clone();
                        self.settings_status = "Settings saved.".into();
                    }
                    Err(err) => self.settings_status = format!("Could not save settings: {err}"),
                }
            }
            ui.label(&self.settings_status);
        });
    }

    fn about(&mut self, ui: &mut egui::Ui) {
        let refresh_due = self
            .avatar_requested
            .map_or(true, |time| time.elapsed() >= Duration::from_secs(3600));
        if refresh_due {
            self.refresh_avatar();
        }
        ui.add_space(35.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(600.0);
            if let Some(texture) = &self.avatar {
                ui.add(egui::Image::new(texture).fit_to_exact_size(egui::vec2(128.0, 128.0)).rounding(64.0));
            } else {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(128.0, 128.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 64.0, ACCENT);
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "TD", egui::FontId::proportional(42.0), Color32::WHITE);
            }
            ui.add_space(10.0);
            ui.heading(avatar::USERNAME);
            ui.hyperlink_to("GitHub profile", avatar::PROFILE_URL);
            ui.label(RichText::new(&self.avatar_status).color(MUTED));
            if ui.add_enabled(self.avatar_receiver.is_none(), egui::Button::new("Refresh avatar")).clicked() { self.refresh_avatar(); }
            ui.add_space(20.0);
            ui.label("Beat Saber Mapping Extractor");
            ui.label("Extract bundle assets and convert compressed map data in one step.");
            ui.hyperlink_to("Project on GitHub", "https://github.com/TheDoctorTTV/Beat-Saber-Mapping-Extractor");
            ui.add_space(10.0);
            ui.small("Uses unity-rs-core for native Unity extraction. Educational use; respect creators’ rights.");
        });
    }
}

impl eframe::App for ExtractorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
        if self.receiver.is_some() || self.avatar_receiver.is_some() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(BACKGROUND)
                    .inner_margin(egui::Margin::symmetric(28.0, 14.0)),
            )
            .show(ctx, |ui| {
                self.header(ui);
                match self.page {
                    Page::Home => self.home(ui),
                    Page::Console => self.console(ui),
                    Page::Settings => self.settings_page(ui),
                    Page::About => self.about(ui),
                }
            });
    }
}

impl Drop for ExtractorApp {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn action(ui: &mut egui::Ui, label: &str, size: [f32; 2]) -> egui::Response {
    ui.add_sized(
        size,
        egui::Button::new(RichText::new(label).size(23.0).color(Color32::WHITE))
            .fill(ACCENT)
            .rounding(22.0)
            .stroke(egui::Stroke::NONE),
    )
}
