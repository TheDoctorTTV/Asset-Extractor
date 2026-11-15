use crate::jobs::{self, Event, Job};
use eframe::egui;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Default, PartialEq)]
enum Page {
    #[default]
    Converter,
    Extractor,
}

#[derive(Default)]
struct Form {
    input: String,
    output: String,
}

pub struct ExtractorApp {
    page: Page,
    converter: Form,
    extractor: Form,
    receiver: Option<Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    progress: f32,
    status: String,
    logs: VecDeque<String>,
    last_output: Option<PathBuf>,
}

impl Default for ExtractorApp {
    fn default() -> Self {
        let output = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|parent| parent.join("extracted_assets")))
            .unwrap_or_else(|| PathBuf::from("extracted_assets"));
        Self {
            page: Page::default(),
            converter: Form::default(),
            extractor: Form {
                input: String::new(),
                output: output.to_string_lossy().into_owned(),
            },
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            progress: 0.0,
            status: "Idle".into(),
            logs: VecDeque::new(),
            last_output: None,
        }
    }
}

impl ExtractorApp {
    fn log(&mut self, line: String) {
        if self.logs.len() >= 2000 {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }

    fn poll(&mut self) {
        loop {
            let event = match self.receiver.as_ref() {
                Some(receiver) => receiver.try_recv(),
                None => break,
            };
            match event {
                Ok(Event::Log(line)) => self.log(line),
                Ok(Event::Progress(value, status)) => {
                    self.progress = value;
                    self.status = status;
                }
                Ok(Event::Finished(status)) => {
                    self.log(status.clone());
                    self.status = status;
                    self.receiver = None;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.status = "Worker stopped unexpectedly".into();
                    self.log(self.status.clone());
                    self.receiver = None;
                    break;
                }
            }
        }
    }

    fn start(&mut self) {
        let form = if self.page == Page::Converter {
            &self.converter
        } else {
            &self.extractor
        };
        if form.input.trim().is_empty() || form.output.trim().is_empty() {
            self.status = "Select an input and an output folder first".into();
            return;
        }
        let input = PathBuf::from(form.input.trim());
        let output = PathBuf::from(form.output.trim());
        if (self.page == Page::Converter && !input.is_dir())
            || (self.page == Page::Extractor && !input.is_file())
        {
            self.status = "Selected input does not exist or is the wrong type".into();
            return;
        }
        let job = match self.page {
            Page::Converter => Job::Convert {
                input,
                output: output.clone(),
            },
            Page::Extractor => Job::Extract {
                input,
                output: output.clone(),
            },
        };
        let (tx, rx) = mpsc::channel();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel);
        match thread::Builder::new()
            .name("asset-worker".into())
            .spawn(move || jobs::run(job, &tx, cancel))
        {
            Ok(_) => {
                self.receiver = Some(rx);
                self.progress = 0.0;
                self.status = "Starting...".into();
                self.logs.clear();
                self.log(format!("Output: {}", output.display()));
                self.last_output = Some(output);
            }
            Err(err) => self.status = format!("Could not start worker: {err}"),
        }
    }
}

impl eframe::App for ExtractorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        let busy = self.receiver.is_some();
        if busy {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        egui::SidePanel::left("navigation")
            .exact_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(20.0);
                ui.heading("Beat Saber");
                ui.label("Mapping Extractor");
                ui.add_space(24.0);
                ui.selectable_value(&mut self.page, Page::Converter, "Converter");
                ui.selectable_value(&mut self.page, Page::Extractor, "Extractor");
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.label(concat!("v", env!("CARGO_PKG_VERSION")));
                    ui.label("Educational use");
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(16.0);
            let converter = self.page == Page::Converter;
            ui.heading(if converter { "GZ.Asset to .DAT Converter" } else { "Unity Asset Bundle Extractor" });
            ui.label(if converter {
                "Convert .gz.asset files directly inside the selected folder."
            } else {
                "Save a raw bundle copy, then export its assets using AssetStudio."
            });
            ui.add_space(20.0);
            ui.add_enabled_ui(!busy, |ui| {
                let form = if converter { &mut self.converter } else { &mut self.extractor };
                ui.label(if converter { "Input folder" } else { "Unity asset bundle file" });
                path_field(ui, "input", &mut form.input, !converter);
                ui.add_space(12.0);
                ui.label("Output folder");
                path_field(ui, "output", &mut form.output, false);
                ui.add_space(8.0);
                ui.small("Existing output files with the same name are replaced after a successful write.");
            });
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!busy, egui::Button::new(if converter { "Start Conversion" } else { "Start Extraction" })).clicked() {
                    self.start();
                }
                if ui.add_enabled(busy && !self.cancel.load(Ordering::Relaxed), egui::Button::new("Cancel")).clicked() {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.status = "Cancelling...".into();
                }
                let output = self.last_output.as_ref().filter(|path| path.is_dir());
                if ui.add_enabled(output.is_some(), egui::Button::new("Open Output Folder")).clicked() {
                    if let Some(output) = output {
                        if let Err(err) = open::that_detached(output) {
                            self.status = format!("Could not open output folder: {err}");
                        }
                    }
                }
            });
            ui.add_space(12.0);
            ui.add(egui::ProgressBar::new(self.progress).show_percentage());
            ui.label(&self.status);
            if busy { ui.small("Progress and log show the active job, even when switching pages."); }
            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                ui.strong("Activity");
                if ui.add_enabled(!busy, egui::Button::new("Clear")).clicked() { self.logs.clear(); }
            });
            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                for line in &self.logs { ui.monospace(line); }
            });
        });
    }
}

impl Drop for ExtractorApp {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn path_field(ui: &mut egui::Ui, id: &str, value: &mut String, file: bool) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(value)
                .id_source(id)
                .desired_width((ui.available_width() - 85.0).max(100.0)),
        );
        if ui.button("Browse...").clicked() {
            let dialog = rfd::FileDialog::new();
            let selected = if file {
                dialog.pick_file()
            } else {
                dialog.pick_folder()
            };
            if let Some(path) = selected {
                *value = path.to_string_lossy().into_owned();
            }
        }
    });
}
