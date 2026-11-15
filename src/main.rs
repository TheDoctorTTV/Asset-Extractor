#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod avatar;
mod jobs;
mod settings;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1105.0, 880.0])
        .with_min_inner_size([720.0, 600.0]);
    if let Ok(icon) = image::load_from_memory_with_format(
        include_bytes!("../beat-saber-map-extractor-icon.ico"),
        image::ImageFormat::Ico,
    ) {
        let rgba = icon.into_rgba8();
        let (width, height) = rgba.dimensions();
        viewport = viewport.with_icon(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        });
    }
    eframe::run_native(
        "Beat Saber Mapping Extractor",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(app::ExtractorApp::new(cc)))),
    )
}
