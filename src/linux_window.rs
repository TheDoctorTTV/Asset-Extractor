//! Keep the application alive independently of its native window.
//!
//! Closing to tray drops the entire eframe window and renderer. Reopening uses
//! eframe's reusable run-on-demand event loop and the same application state.
//! This works on native Wayland without attempting unsupported visibility calls.
use crate::app::ExtractorApp;
use eframe::egui;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use std::{cell::RefCell, rc::Rc};
use wayland_client::{
    backend::{Backend, ObjectId},
    protocol::wl_surface::WlSurface,
    Connection, Proxy,
};

type SharedApp = Rc<RefCell<Option<ExtractorApp>>>;

struct Window {
    app: SharedApp,
    surface: Option<WaylandSurface>,
}

// A borrowed Wayland surface, used only while eframe still owns the native
// window. It never dispatches events or destroys winit's protocol objects.
struct WaylandSurface {
    connection: Connection,
    surface: WlSurface,
}

impl WaylandSurface {
    fn from_context(
        cc: &eframe::CreationContext<'_>,
    ) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(window)) =
            (cc.display_handle()?.as_raw(), cc.window_handle()?.as_raw())
        else {
            return Ok(None);
        };
        // SAFETY: CreationContext exposes the live display/surface owned by
        // eframe. Both remain valid through App::on_exit, where we take and
        // drop this borrowed connection before eframe destroys its window.
        let (backend, id) = unsafe {
            (
                Backend::from_foreign_display(display.display.as_ptr().cast()),
                ObjectId::from_ptr(WlSurface::interface(), window.surface.as_ptr().cast())?,
            )
        };
        let connection = Connection::from_backend(backend);
        let surface = WlSurface::from_id(&connection, id)?;
        Ok(Some(Self {
            connection,
            surface,
        }))
    }

    fn unmap(self) {
        // The final frame has already been presented. Detaching its buffer
        // removes the toplevel immediately, even if winit defers object cleanup
        // until the next run-on-demand session. Do not remap this old surface.
        self.surface.attach(None, 0, 0);
        self.surface.commit();
        if let Err(error) = self.connection.flush() {
            eprintln!("Could not flush Wayland window shutdown: {error}");
        }
    }
}

impl eframe::App for Window {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(app) = self.app.borrow_mut().as_mut() {
            eframe::App::update(app, ctx, frame);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(surface) = self.surface.take() {
            surface.unmap();
        }
    }
}

pub fn run(viewport: egui::ViewportBuilder) -> eframe::Result<()> {
    let app: SharedApp = Rc::new(RefCell::new(None));
    loop {
        let mut viewport = viewport.clone();
        if let Some(app) = app.borrow().as_ref() {
            viewport = viewport.with_inner_size(app.window_size());
        }
        let window_app = Rc::clone(&app);
        eframe::run_native(
            "Asset Extractor",
            eframe::NativeOptions {
                viewport,
                run_and_return: true,
                ..Default::default()
            },
            Box::new(move |cc| {
                let surface = WaylandSurface::from_context(cc)?;
                {
                    let mut state = window_app.borrow_mut();
                    if let Some(app) = state.as_mut() {
                        app.attach_window(cc);
                    } else {
                        *state = Some(ExtractorApp::new(cc));
                    }
                }
                Ok(Box::new(Window {
                    app: window_app,
                    surface,
                }))
            }),
        )?;

        // The Window wrapper has been dropped, but the app, tray and workers
        // remain owned here. A normal close or Quit ends the process instead.
        let reopen = app
            .borrow_mut()
            .as_mut()
            .is_some_and(ExtractorApp::wait_in_tray);
        if !reopen {
            return Ok(());
        }
    }
}
