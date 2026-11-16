use crate::{
    avatar,
    jobs::{self, Destination, Event},
    settings::Settings,
    theme::{self, Theme},
    tray::{self, Tray},
    updater::Updater,
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

#[derive(Clone, Copy, Default, PartialEq)]
enum Page {
    #[default]
    Home,
    Console,
    Settings,
    About,
}

struct ExtractionConflict {
    input: PathBuf,
    output: PathBuf,
    existing_folder_name: String,
    new_folder_name: String,
    error: String,
}

pub struct ExtractorApp {
    page: Page,
    updater: Updater,
    bundle: String,
    settings: Settings,
    draft: Settings,
    settings_status: String,
    tray: Option<Tray>,
    tray_attempted: bool,
    tray_actions: tray::Actions,
    window_in_tray: bool,
    #[cfg(target_os = "linux")]
    window_size: [f32; 2],
    receiver: Option<Receiver<Event>>,
    conflict: Option<ExtractionConflict>,
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
    pub fn new(cc: &eframe::CreationContext<'_>, instance: &crate::single_instance::Instance) -> Self {
        let (settings, settings_status) = match Settings::load() {
            Ok(settings) => (settings, String::new()),
            Err(err) => (
                Settings::default(),
                format!("Could not load settings: {err}"),
            ),
        };
        theme::install_fonts(&cc.egui_ctx);
        settings.theme.apply(&cc.egui_ctx);
        let tray_actions = tray::Actions::new(cc.egui_ctx.clone());
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = cc.window_handle() {
                if let RawWindowHandle::Win32(window) = handle.as_raw() {
                    tray_actions.set_window(window.hwnd.get());
                }
            }
        }
        instance.attach(tray_actions.clone());
        let mut logs = VecDeque::new();
        if !settings_status.is_empty() {
            logs.push_back(settings_status.clone());
        }
        Self {
            page: Page::Home,
            updater: Updater::new(
                settings.update_channel,
                settings.update_check_interval,
                settings.automatic_updates,
            ),
            bundle: String::new(),
            draft: settings.clone(),
            settings,
            settings_status,
            tray: None,
            tray_attempted: false,
            tray_actions,
            window_in_tray: false,
            #[cfg(target_os = "linux")]
            window_size: [1105.0, 880.0],
            receiver: None,
            conflict: None,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: 0.0,
            status: "Select a bundle to extract its assets.".into(),
            logs,
            last_output: None,
            avatar: None,
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
                            self.avatar = None;
                            self.avatar_status = "Avatar unavailable.".into();
                            self.log(format!("GitHub avatar: {err}"));
                        }
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    self.avatar_receiver = None;
                    self.avatar_status = "Avatar unavailable.".into();
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
        if self.receiver.is_some() || self.conflict.is_some() || self.updater.installing {
            return;
        }
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
        let folder_name = jobs::bundle_folder_name(&input);
        match jobs::destination_exists(&output.join(&folder_name)) {
            Ok(true) => {
                let mut index = 2;
                let mut suggested_base = folder_name.clone();
                while suggested_base.len() > 170 {
                    suggested_base.pop();
                }
                let new_folder_name = loop {
                    let name = format!("{suggested_base}-{index}");
                    match jobs::destination_exists(&output.join(&name)) {
                        Ok(false) => break name,
                        Ok(true) => index += 1,
                        Err(err) => {
                            self.status = format!("Could not check the destination: {err}");
                            return;
                        }
                    }
                };
                self.conflict = Some(ExtractionConflict {
                    input,
                    output,
                    existing_folder_name: folder_name,
                    new_folder_name,
                    error: String::new(),
                });
            }
            Ok(false) => self.launch(
                input,
                output,
                Destination {
                    folder_name,
                    overwrite: false,
                },
            ),
            Err(err) => self.status = format!("Could not check the destination: {err}"),
        }
    }

    fn launch(&mut self, input: PathBuf, output: PathBuf, destination: Destination) {
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        let (tx, rx) = mpsc::channel();
        let input_label = input.display().to_string();
        let output_label = output.join(&destination.folder_name).display().to_string();
        match thread::Builder::new()
            .name("asset-extractor".into())
            .spawn(move || jobs::run(input, output, destination, tx, cancel))
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

    fn conflict_prompt(&mut self, ctx: &egui::Context) {
        let Some(mut conflict) = self.conflict.take() else {
            return;
        };
        let mut open = true;
        let mut choice = None;
        let mut cancel_prompt = false;
        let previous = conflict.output.join(&conflict.existing_folder_name);
        let can_overwrite = std::fs::symlink_metadata(&previous)
            .map(|meta| meta.is_dir() && !meta.file_type().is_symlink())
            .unwrap_or(false);
        egui::Window::new("Extraction already exists")
            .id(egui::Id::new("extraction-conflict"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.label("This destination already exists:");
                ui.label(previous.display().to_string());
                ui.add_space(8.0);
                ui.label("Save in a new folder:");
                ui.add(
                    egui::TextEdit::singleline(&mut conflict.new_folder_name).desired_width(350.0),
                );
                if ui.button("Extract to new folder").clicked() {
                    let validation = jobs::validate_folder_name(&conflict.new_folder_name)
                        .and_then(|()| {
                            if jobs::destination_exists(
                                &conflict.output.join(&conflict.new_folder_name),
                            )? {
                                Err(std::io::Error::other(
                                    "That folder name is already in use. Enter a different name.",
                                ))
                            } else {
                                Ok(())
                            }
                        });
                    match validation {
                        Ok(()) => choice = Some(false),
                        Err(err) => conflict.error = err.to_string(),
                    }
                }
                ui.separator();
                ui.label("Overwrite replaces all files in the previous extraction folder.");
                if ui
                    .add_enabled(
                        can_overwrite,
                        egui::Button::new("Overwrite previous extraction"),
                    )
                    .clicked()
                {
                    choice = Some(true);
                }
                if !can_overwrite {
                    ui.small(
                        "Only a regular directory can be overwritten. Choose a new folder name.",
                    );
                }
                if !conflict.error.is_empty() {
                    ui.label(RichText::new(&conflict.error).color(ui.visuals().error_fg_color));
                }
                if ui.button("Cancel").clicked() {
                    cancel_prompt = true;
                }
            });
        if let Some(overwrite) = choice {
            let folder_name = if overwrite {
                conflict.existing_folder_name
            } else {
                conflict.new_folder_name
            };
            self.launch(
                conflict.input,
                conflict.output,
                Destination {
                    folder_name,
                    overwrite,
                },
            );
        } else if open && !cancel_prompt {
            self.conflict = Some(conflict);
        } else {
            self.status = "Extraction cancelled before starting.".into();
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
        let palette = self.settings.theme.palette();
        ui.add_space(9.0);
        ui.horizontal(|ui| {
            let title_size = if ui.available_width() < 836.0 {
                38.0
            } else {
                48.0
            };
            let title_width = ui.fonts(|fonts| {
                fonts
                    .layout_no_wrap(
                        "Asset Extractor".into(),
                        egui::FontId::proportional(title_size),
                        Color32::WHITE,
                    )
                    .size()
                    .x
                    + fonts
                        .layout_no_wrap(
                            concat!("v", env!("CARGO_PKG_VERSION")).into(),
                            egui::FontId::proportional(11.0),
                            palette.muted,
                        )
                        .size()
                        .x
            }) + ui.spacing().item_spacing.x;
            ui.add_space(((ui.available_width() - title_width) / 2.0).max(0.0));
            ui.label(RichText::new("Asset Extractor").size(title_size));
            ui.label(
                RichText::new(concat!("v", env!("CARGO_PKG_VERSION")))
                    .size(11.0)
                    .color(palette.muted),
            );
        });
        ui.add_space(17.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 15.0;
            let width = 111.0_f32.min(((ui.available_width() - 45.0) / 4.0).max(60.0));
            ui.add_space(((ui.available_width() - (width * 4.0 + 45.0)) / 2.0).max(0.0));
            for (page, label) in [
                (Page::Home, "Home"),
                (Page::Console, "Console"),
                (Page::Settings, "Settings"),
                (Page::About, "About"),
            ] {
                let active = self.page == page;
                let response = theme::button(ui, label, [width, 36.0], active, 12.0);
                if page == Page::About && self.updater.update_available() {
                    ui.painter().circle_filled(
                        response.rect.right_top() + egui::vec2(-10.0, 10.0),
                        4.0,
                        ui.visuals().error_fg_color,
                    );
                }
                if response.clicked() {
                    self.page = page;
                }
            }
        });
        ui.add_space(17.0);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 2.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, palette.border);
    }

    fn home(&mut self, ui: &mut egui::Ui) {
        let palette = self.settings.theme.palette();
        let busy = self.receiver.is_some();
        ui.add_space(55.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(818.0);
            ui.add_enabled_ui(!busy && !self.updater.installing, |ui| {
                ui.label("Unity asset bundle");
                ui.horizontal(|ui| {
                    let field_width = (ui.available_width() - 147.0).max(150.0);
                    ui.add_sized(
                        [field_width, 38.0],
                        egui::TextEdit::singleline(&mut self.bundle)
                            .hint_text("Select a Unity bundle")
                            .margin(egui::vec2(16.0, 9.0))
                            .vertical_align(egui::Align::Center),
                    );
                    if theme::button(ui, "Browse", [135.0, 38.0], false, 13.0).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Select a Unity asset bundle")
                            .pick_file()
                        {
                            self.bundle = path.to_string_lossy().into_owned();
                        }
                    }
                });
                ui.add_space(26.0);
                if action(ui, "Extract", [180.0, 58.0]).clicked() {
                    self.start();
                }
            });
            ui.add_space(16.0);
            if busy {
                ui.add(
                    egui::ProgressBar::new(self.progress)
                        .fill(palette.primary)
                        .animate(true)
                        .desired_width(400.0),
                );
                ui.spinner();
            }
            ui.label(RichText::new(&self.status).color(palette.muted));
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
        let palette = self.settings.theme.palette();
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
        egui::Frame::none()
            .fill(palette.surface)
            .rounding(16.0)
            .inner_margin(20.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.logs.is_empty() {
                            ui.label(
                                RichText::new("Extraction activity will appear here.")
                                    .color(palette.muted),
                            );
                        }
                        for line in &self.logs {
                            ui.monospace(line);
                        }
                    });
            });
    }

    fn settings_page(&mut self, ui: &mut egui::Ui) {
        let palette = self.settings.theme.palette();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.add_space(32.0);
        ui.vertical_centered(|ui| {
            ui.set_max_width(818.0);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.heading("Settings");
            ui.label("Theme");
            let previous = self.draft.theme;
            egui::ComboBox::from_id_salt("theme")
                .selected_text(self.draft.theme.label())
                .width(240.0)
                .show_ui(ui, |ui| {
                    for theme in Theme::ALL {
                        ui.selectable_value(&mut self.draft.theme, theme, theme.label());
                    }
                });
            if self.draft.theme != previous {
                self.settings.theme = self.draft.theme;
                self.settings.theme.apply(ui.ctx());
                ui.ctx().request_repaint();
                self.settings_status = match self.settings.save() {
                    Ok(()) => "Theme saved.".into(),
                    Err(err) => format!("Theme applied, but could not save it: {err}"),
                };
            }
            ui.small("Theme changes apply and save automatically.");
            ui.add_space(18.0);
            ui.separator();
            ui.add_space(12.0);
            ui.heading("Window behavior");
            if ui.checkbox(&mut self.draft.close_to_tray, "Close to system tray").changed() {
                self.settings.close_to_tray = self.draft.close_to_tray;
                ui.ctx().request_repaint();
                self.settings_status = match self.settings.save() {
                    Ok(()) => "Window behavior saved.".into(),
                    Err(err) => format!("Window behavior applied, but could not save it: {err}"),
                };
            }
            ui.small("Closing the window keeps the app running. Use the tray menu to show it again or quit.");
            ui.add_space(18.0);
            ui.separator();
            ui.add_space(12.0);
            ui.heading("Save location");
            ui.add_space(12.0);
            ui.checkbox(
                &mut self.draft.use_default_folder,
                "Always use this output folder",
            );
            ui.add_space(8.0);
            ui.add_enabled_ui(self.draft.use_default_folder, |ui| {
                ui.horizontal(|ui| {
                    let browse_width = 100.0;
                    let field_width = (ui.available_width()
                        - browse_width
                        - ui.spacing().item_spacing.x)
                        .clamp(150.0, 400.0);
                    ui.add_sized(
                        [field_width, 32.0],
                        egui::TextEdit::singleline(&mut self.draft.default_folder)
                            .hint_text("Output folder")
                            .margin(egui::vec2(16.0, 7.0))
                            .vertical_align(egui::Align::Center),
                    );
                    if ui
                        .add_sized([browse_width, 32.0], egui::Button::new("Browse..."))
                        .clicked()
                    {
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
                    .color(palette.muted),
            );
            ui.small("Each bundle gets its own folder. Existing folders prompt for a new name or overwrite.");
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
        });
        });
    }

    fn about(&mut self, ui: &mut egui::Ui) {
        let palette = self.settings.theme.palette();
        let refresh_due = self
            .avatar_requested
            .map_or(true, |time| time.elapsed() >= Duration::from_secs(3600));
        if refresh_due {
            self.refresh_avatar();
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(22.0);
                ui.heading("About");
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if let Some(texture) = &self.avatar {
                        ui.add(
                            egui::Image::new(texture)
                                .fit_to_exact_size(egui::vec2(48.0, 48.0))
                                .rounding(24.0),
                        );
                    } else {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
                        ui.painter()
                            .circle_filled(rect.center(), 24.0, palette.primary);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "TD",
                            egui::FontId::proportional(18.0),
                            Color32::WHITE,
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Created by");
                        ui.hyperlink_to(
                            RichText::new("TheDoctorTTV").strong(),
                            "https://github.com/TheDoctorTTV/Asset-Extractor",
                        )
                        .on_hover_text("Asset-Extractor");
                    });
                });
                if !self.updater.enabled() {
                    ui.add_space(16.0);
                    ui.label("Updates for this AppImage are managed by Gear Lever.");
                    return;
                }
                ui.add_space(26.0);
                ui.separator();
                ui.add_space(16.0);
                let preferences_changed = self
                    .updater
                    .show(ui, self.receiver.is_some() || self.conflict.is_some());
                if preferences_changed {
                    self.settings.update_channel = self.updater.channel;
                    self.settings.update_check_interval = self.updater.interval;
                    self.draft.update_channel = self.updater.channel;
                    self.draft.update_check_interval = self.updater.interval;
                    self.settings.automatic_updates = self.updater.automatic_updates;
                    self.draft.automatic_updates = self.updater.automatic_updates;
                    if let Err(err) = self.settings.save() {
                        self.updater.status = format!("Could not save update preferences: {err}");
                        self.log(self.updater.status.clone());
                    }
                }
            });
    }
}

impl ExtractorApp {
    fn show_window(&mut self, ctx: &egui::Context) {
        self.window_in_tray = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }

    #[cfg(target_os = "linux")]
    pub fn window_size(&self) -> [f32; 2] {
        self.window_size
    }

    #[cfg(target_os = "linux")]
    pub fn attach_window(&mut self, cc: &eframe::CreationContext<'_>) {
        theme::install_fonts(&cc.egui_ctx);
        self.settings.theme.apply(&cc.egui_ctx);
        self.tray_actions.set_context(Some(cc.egui_ctx.clone()));
        self.window_in_tray = false;
        // Texture handles belong to the old renderer. Reload the optional avatar
        // while retaining all application data, workers and update state.
        self.avatar = None;
        self.avatar_requested = None;
    }

    #[cfg(target_os = "linux")]
    pub fn wait_in_tray(&mut self) -> bool {
        if !self.window_in_tray || self.tray_actions.is_quitting() {
            return false;
        }
        // No graphical window or renderer exists during this loop. Tray callbacks
        // wake it directly; extraction/update channels are serviced periodically.
        self.tray_actions.set_context(None);
        let ctx = egui::Context::default();
        while self.window_in_tray
            && !self.tray_actions.is_quitting()
            && self.settings.close_to_tray
        {
            self.background_tick(&ctx);
            if self.conflict.is_some() {
                self.show_window(&ctx);
            }
            if self.window_in_tray && !self.tray_actions.is_quitting() {
                self.tray_actions.wait(Duration::from_millis(100));
            }
        }
        !self.tray_actions.is_quitting()
    }

    fn background_tick(&mut self, ctx: &egui::Context) {
        // Start the tray when enabled, rather than waiting for the first close.
        if self.settings.close_to_tray && !self.tray_attempted {
            self.tray_attempted = true;
            match Tray::new(self.tray_actions.clone()) {
                Ok(tray) => self.tray = Some(tray),
                Err(err) => {
                    self.settings_status = format!("Could not open the system tray: {err}. Disable Close to system tray to exit normally.");
                    self.log(self.settings_status.clone());
                }
            }
        } else if !self.settings.close_to_tray {
            self.tray = None;
            self.tray_attempted = false;
        }
        // Tray callbacks wake either the window event loop or the Linux tray
        // wait loop. All application and window changes stay on the main thread.
        for action in self.tray_actions.take() {
            match action {
                tray::Action::Show | tray::Action::Unavailable => {
                    self.show_window(ctx);
                    if matches!(action, tray::Action::Unavailable) {
                        self.settings_status = "System tray unavailable. The window will stay open; disable Close to system tray to exit normally.".into();
                        self.log(self.settings_status.clone());
                        self.page = Page::Settings;
                    }
                }
                tray::Action::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                tray::Action::Toggle => {
                    if self.window_in_tray {
                        self.show_window(ctx);
                    } else if self.settings.close_to_tray
                        && self.tray.as_ref().is_some_and(Tray::available)
                    {
                        self.window_in_tray = true;
                        #[cfg(target_os = "linux")]
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        #[cfg(not(target_os = "linux"))]
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    }
                }
                tray::Action::CheckUpdates => {
                    self.page = Page::About;
                    self.updater.check();
                    self.show_window(ctx);
                }
                tray::Action::VisitWebsite | tray::Action::OpenRepository => {
                    let url = match action {
                        tray::Action::VisitWebsite => "https://thetimevortex.net",
                        _ => "https://github.com/TheDoctorTTV/Asset-Extractor",
                    };
                    if let Err(err) = open::that_detached(url) {
                        self.log(format!("Could not open {url}: {err}"));
                    }
                }
            }
        }
        self.poll(ctx);
        let update_status = self.updater.status.clone();
        let restart = self
            .updater
            .tick(ctx, self.receiver.is_some() || self.conflict.is_some());
        if self.updater.status != update_status && !self.updater.installing {
            self.log(format!("Updates: {}", self.updater.status));
        }
        if let Some(path) = restart {
            let mut command = std::process::Command::new(path);
            crate::single_instance::Instance::configure_restart(&mut command);
            #[cfg(target_os = "linux")]
            {
                // The replacement runtime must establish its own AppImage paths.
                if let Some(app_dir) = std::env::var_os("APPDIR") {
                    let app_dir = std::path::PathBuf::from(app_dir);
                    for variable in ["LD_LIBRARY_PATH", "PATH"] {
                        if let Some(value) = std::env::var_os(variable) {
                            let paths: Vec<_> = std::env::split_paths(&value)
                                .filter(|entry| !entry.starts_with(&app_dir))
                                .collect();
                            if let Ok(value) = std::env::join_paths(paths) {
                                command.env(variable, value);
                            }
                        }
                    }
                }
                for variable in ["APPIMAGE", "APPDIR", "ARGV0"] {
                    command.env_remove(variable);
                }
            }
            match command.spawn() {
                Ok(_) => self.tray_actions.quit(),
                Err(err) => {
                    self.updater.status = format!("Update installed. Close and reopen the app to finish; automatic restart failed: {err}");
                    self.log(self.updater.status.clone());
                }
            }
        }
        if self.receiver.is_some() || self.avatar_receiver.is_some() {
            ctx.request_repaint_after(Duration::from_millis(80));
        }
    }
}

impl eframe::App for ExtractorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.background_tick(ctx);
        #[cfg(target_os = "linux")]
        if let Some(rect) = ctx.input(|input| input.viewport().inner_rect) {
            self.window_size = [rect.width().max(720.0), rect.height().max(600.0)];
        }
        if ctx.input(|input| input.viewport().close_requested())
            && self.settings.close_to_tray
            && !self.tray_actions.is_quitting()
        {
            if self.tray.as_ref().is_some_and(Tray::available) {
                self.window_in_tray = true;
                // Linux lets eframe destroy the native window. Its host keeps
                // this app alive and recreates the window on tray activation.
                #[cfg(not(target_os = "linux"))]
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                }
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.settings_status = "System tray unavailable. The window will stay open; disable Close to system tray to exit normally.".into();
                self.log(self.settings_status.clone());
                self.page = Page::Settings;
            }
        }
        let palette = self.settings.theme.palette();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(palette.background)
                    .inner_margin(egui::Margin::symmetric(32.0, 14.0)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.set_max_width(978.0);
                    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.add_enabled_ui(self.conflict.is_none(), |ui| {
                            self.header(ui);
                            match self.page {
                                Page::Home => self.home(ui),
                                Page::Console => self.console(ui),
                                Page::Settings => self.settings_page(ui),
                                Page::About => self.about(ui),
                            }
                        });
                    });
                });
            });
        self.conflict_prompt(ctx);
    }
}

impl Drop for ExtractorApp {
    fn drop(&mut self) {
        self.tray_actions.mark_quitting();
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn action(ui: &mut egui::Ui, label: &str, size: [f32; 2]) -> egui::Response {
    theme::button(
        ui,
        label,
        size,
        true,
        if size[1] >= 50.0 { 23.0 } else { 13.0 },
    )
}
