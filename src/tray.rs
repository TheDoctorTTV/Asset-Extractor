use eframe::egui;
#[cfg(target_os = "windows")]
use std::sync::atomic::AtomicIsize;
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    Arc, Mutex,
};

#[derive(Clone)]
pub struct Actions {
    ctx: Arc<Mutex<Option<egui::Context>>>,
    quitting: Arc<AtomicBool>,
    pending: Arc<AtomicU8>,
    #[cfg(target_os = "windows")]
    window: Arc<AtomicIsize>,
    #[cfg(target_os = "linux")]
    wake: Arc<(Mutex<()>, std::sync::Condvar)>,
}

#[derive(Clone, Copy)]
pub enum Action {
    Show,
    Toggle,
    CheckUpdates,
    VisitWebsite,
    OpenRepository,
    Quit,
    Unavailable,
}

impl Actions {
    pub fn new(ctx: egui::Context) -> Self {
        Self {
            ctx: Arc::new(Mutex::new(Some(ctx))),
            quitting: Arc::new(AtomicBool::new(false)),
            pending: Arc::new(AtomicU8::new(0)),
            #[cfg(target_os = "windows")]
            window: Arc::new(AtomicIsize::new(0)),
            #[cfg(target_os = "linux")]
            wake: Arc::new((Mutex::new(()), std::sync::Condvar::new())),
        }
    }

    pub fn show(&self) {
        self.send(1);
    }

    pub fn toggle(&self) {
        self.send(64);
    }

    pub fn quit(&self) {
        self.mark_quitting();
        self.send(2);
    }

    pub fn mark_quitting(&self) {
        self.quitting.store(true, Ordering::Relaxed);
    }

    #[cfg(target_os = "windows")]
    pub fn set_window(&self, window: isize) {
        self.window.store(window, Ordering::Relaxed);
    }

    pub fn check_updates(&self) {
        self.send(8);
    }

    pub fn visit_website(&self) {
        self.send(16);
    }

    pub fn open_repository(&self) {
        self.send(32);
    }

    #[cfg(target_os = "linux")]
    fn unavailable(&self) {
        self.send(4);
    }

    fn send(&self, bit: u8) {
        {
            #[cfg(target_os = "linux")]
            let _guard = self.wake.0.lock().unwrap_or_else(|err| err.into_inner());
            self.pending.fetch_or(bit, Ordering::Relaxed);
            #[cfg(target_os = "linux")]
            self.wake.1.notify_one();
        }
        let ctx = self
            .ctx
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .clone();
        if let Some(ctx) = ctx {
            ctx.request_repaint();
        }
        #[cfg(target_os = "windows")]
        {
            // Normal redraw scheduling can wait for a visible window. Posting
            // a paint message explicitly also delivers Show/Quit/IPC actions
            // while the native window is hidden in the tray.
            let window = self.window.load(Ordering::Relaxed);
            if window != 0 {
                // SAFETY: PostMessageW queues a message without dereferencing
                // the HWND here; an already-destroyed handle simply fails.
                unsafe {
                    windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                        window as *mut std::ffi::c_void,
                        windows_sys::Win32::UI::WindowsAndMessaging::WM_PAINT,
                        0,
                        0,
                    );
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    pub fn set_context(&self, ctx: Option<egui::Context>) {
        *self.ctx.lock().unwrap_or_else(|err| err.into_inner()) = ctx;
    }

    #[cfg(target_os = "linux")]
    pub fn wait(&self, timeout: std::time::Duration) {
        let guard = self.wake.0.lock().unwrap_or_else(|err| err.into_inner());
        drop(self.wake.1.wait_timeout_while(guard, timeout, |_| {
            self.pending.load(Ordering::Relaxed) == 0 && !self.is_quitting()
        }));
    }

    pub fn take(&self) -> Vec<Action> {
        let pending = self.pending.swap(0, Ordering::Relaxed);
        let mut actions = Vec::new();
        if pending & 1 != 0 {
            actions.push(Action::Show);
        }
        // Explicit activation (another launch, Show, or update check) wins over
        // a simultaneous tray toggle so it never leaves the requested UI hidden.
        if pending & 64 != 0 && pending & (1 | 4 | 8) == 0 {
            actions.push(Action::Toggle);
        }
        if pending & 8 != 0 {
            actions.push(Action::CheckUpdates);
        }
        if pending & 16 != 0 {
            actions.push(Action::VisitWebsite);
        }
        if pending & 32 != 0 {
            actions.push(Action::OpenRepository);
        }
        if pending & 4 != 0 {
            actions.push(Action::Unavailable);
        }
        if pending & 2 != 0 {
            actions.push(Action::Quit);
        }
        actions
    }

    pub fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::Relaxed)
    }
}

#[cfg(target_os = "linux")]
pub struct Tray(ksni::blocking::Handle<LinuxTray>);

#[cfg(target_os = "linux")]
struct LinuxTray {
    actions: Actions,
    icon: ksni::Icon,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for LinuxTray {
    fn id(&self) -> String {
        "io.github.TheDoctorTTV.AssetExtractor".into()
    }

    fn title(&self) -> String {
        "Asset Extractor".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![self.icon.clone()]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.actions.toggle();
    }

    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        // Restore the window if its tray host disappears while it is hidden.
        self.actions.unavailable();
        false
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: "Show Asset Extractor".into(),
                activate: Box::new(|tray: &mut Self| tray.actions.show()),
                ..Default::default()
            }
            .into(),
            ksni::menu::StandardItem {
                label: "Check for updates".into(),
                activate: Box::new(|tray: &mut Self| tray.actions.check_updates()),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            ksni::menu::StandardItem {
                label: "Visit thetimevortex.net".into(),
                activate: Box::new(|tray: &mut Self| tray.actions.visit_website()),
                ..Default::default()
            }
            .into(),
            ksni::menu::StandardItem {
                label: "Asset Extractor on GitHub".into(),
                activate: Box::new(|tray: &mut Self| tray.actions.open_repository()),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            ksni::menu::StandardItem {
                label: "Quit".into(),
                activate: Box::new(|tray: &mut Self| tray.actions.quit()),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(target_os = "linux")]
impl Tray {
    pub fn new(actions: Actions) -> Result<Self, String> {
        use ksni::blocking::TrayMethods;
        let image = image::load_from_memory(include_bytes!(
            "../assets/io.github.TheDoctorTTV.AssetExtractor.png"
        ))
        .map_err(|err| err.to_string())?
        .into_rgba8();
        let icon = ksni::Icon {
            width: image.width() as i32,
            height: image.height() as i32,
            data: image
                .pixels()
                .flat_map(|p| [p[3], p[0], p[1], p[2]])
                .collect(),
        };
        LinuxTray { actions, icon }
            .spawn()
            .map(Self)
            .map_err(|err| err.to_string())
    }

    pub fn available(&self) -> bool {
        !self.0.is_closed()
    }
}

#[cfg(target_os = "linux")]
impl Drop for Tray {
    fn drop(&mut self) {
        let _ = self.0.shutdown();
    }
}

#[cfg(target_os = "windows")]
pub struct Tray {
    _icon: tray_icon::TrayIcon,
}

#[cfg(target_os = "windows")]
impl Tray {
    pub fn new(actions: Actions) -> Result<Self, String> {
        use tray_icon::{
            menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
            Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent,
        };
        let menu = Menu::new();
        // Event handlers live for the process lifetime; stable IDs allow the
        // icon to be disabled and recreated without stale menu callbacks.
        let show = MenuItem::with_id("asset-extractor-show", "Show Asset Extractor", true, None);
        let check = MenuItem::with_id(
            "asset-extractor-check-updates",
            "Check for updates",
            true,
            None,
        );
        let website = MenuItem::with_id(
            "asset-extractor-website",
            "Visit thetimevortex.net",
            true,
            None,
        );
        let repository = MenuItem::with_id(
            "asset-extractor-github",
            "Asset Extractor on GitHub",
            true,
            None,
        );
        let quit = MenuItem::with_id("asset-extractor-quit", "Quit", true, None);
        menu.append_items(&[
            &show,
            &check,
            &PredefinedMenuItem::separator(),
            &website,
            &repository,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|err| err.to_string())?;
        let image = image::load_from_memory(include_bytes!(
            "../assets/io.github.TheDoctorTTV.AssetExtractor.png"
        ))
        .map_err(|err| err.to_string())?
        .into_rgba8();
        let (width, height) = image.dimensions();
        let icon =
            Icon::from_rgba(image.into_raw(), width, height).map_err(|err| err.to_string())?;
        let show_id = show.id().clone();
        let check_id = check.id().clone();
        let website_id = website.id().clone();
        let repository_id = repository.id().clone();
        let quit_id = quit.id().clone();
        let click_actions = actions.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                click_actions.toggle();
            }
        }));
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == show_id {
                actions.show();
            } else if event.id == check_id {
                actions.check_updates();
            } else if event.id == website_id {
                actions.visit_website();
            } else if event.id == repository_id {
                actions.open_repository();
            } else if event.id == quit_id {
                actions.quit();
            }
        }));
        // Install process-lifetime handlers before showing the icon, so an
        // early native event cannot initialize the crate's handler to None.
        let icon = TrayIconBuilder::new()
            .with_tooltip("Asset Extractor")
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_icon(icon)
            .build()
            .map_err(|err| err.to_string())?;
        Ok(Self { _icon: icon })
    }

    pub fn available(&self) -> bool {
        true
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub struct Tray;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl Tray {
    pub fn new(_actions: Actions) -> Result<Self, String> {
        Err("System tray support is unavailable on this platform".into())
    }

    pub fn available(&self) -> bool {
        false
    }
}
