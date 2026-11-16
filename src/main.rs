#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod avatar;
#[cfg(target_os = "linux")]
mod desktop;
mod environments;
mod jobs;
#[cfg(target_os = "linux")]
mod linux_window;
mod map_info;
mod settings;
mod theme;
mod tray;
mod updater;

use eframe::egui;

fn main() -> eframe::Result<()> {
    #[cfg(target_os = "linux")]
    if let Err(error) = desktop::register() {
        eprintln!("Could not register desktop icon: {error}");
    }
    let mut viewport = egui::ViewportBuilder::default()
        .with_app_id("io.github.TheDoctorTTV.AssetExtractor")
        .with_inner_size([1105.0, 880.0])
        .with_min_inner_size([720.0, 600.0]);
    if let Ok(icon) = image::load_from_memory_with_format(
        include_bytes!("../asset-extractor-icon.ico"),
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
    #[cfg(target_os = "linux")]
    {
        linux_window::run(viewport)
    }
    #[cfg(not(target_os = "linux"))]
    eframe::run_native(
        "Asset Extractor",
        eframe::NativeOptions {
            viewport,
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(app::ExtractorApp::new(cc)))),
    )
}
