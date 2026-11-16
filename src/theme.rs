use eframe::egui::{self, Color32, FontId, RichText, Stroke};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Amber,
    #[serde(rename = "blue")]
    Cobalt,
    #[serde(rename = "orange")]
    Ember,
    #[serde(rename = "emerald")]
    Evergreen,
    #[serde(rename = "mono")]
    Graphite,
    #[serde(rename = "teal")]
    Lagoon,
    #[serde(rename = "rose")]
    RoseQuartz,
    #[serde(rename = "red")]
    Ruby,
    #[default]
    #[serde(rename = "purple", alias = "current", other)]
    Amethyst,
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub background: Color32,
    pub surface: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub border: Color32,
    pub ring: Color32,
}

const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

impl Theme {
    pub const ALL: [Self; 9] = [
        Self::Amber,
        Self::Amethyst,
        Self::Cobalt,
        Self::Ember,
        Self::Evergreen,
        Self::Graphite,
        Self::Lagoon,
        Self::RoseQuartz,
        Self::Ruby,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Amber => "Amber",
            Self::Amethyst => "Amethyst",
            Self::Cobalt => "Cobalt",
            Self::Ember => "Ember",
            Self::Evergreen => "Evergreen",
            Self::Graphite => "Graphite",
            Self::Lagoon => "Lagoon",
            Self::RoseQuartz => "Rose Quartz",
            Self::Ruby => "Ruby",
        }
    }

    // Exact palette tokens from stream247's web/style.css.
    pub fn palette(self) -> Palette {
        let [background, surface, accent, accent_hover, primary, primary_hover, ring] = match self {
            Self::Amethyst => [
                0x35263e, 0x2e2037, 0x6515ad, 0x7924c5, 0xa617f5, 0xb637ff, 0xd69dff,
            ],
            Self::Cobalt => [
                0x233047, 0x1c273b, 0x2059ac, 0x2b6ac5, 0x2586f5, 0x4b9fff, 0x91c9ff,
            ],
            Self::Ruby => [
                0x3e262d, 0x321e25, 0xa52a47, 0xbb3655, 0xe54b70, 0xf76587, 0xffb3c3,
            ],
            Self::Graphite => [
                0x2e2b31, 0x242127, 0x55515e, 0x686473, 0x77717f, 0x8b8494, 0xddd5e6,
            ],
            Self::Evergreen => [
                0x22382e, 0x1a2d24, 0x245b40, 0x2c6c4d, 0x16804f, 0x198353, 0x8ce0b0,
            ],
            Self::Lagoon => [
                0x203638, 0x192b2e, 0x205b60, 0x286c71, 0x147c80, 0x148184, 0x8cdee0,
            ],
            Self::Amber => [
                0x383023, 0x2d261b, 0x6c4c17, 0x805b20, 0x956300, 0xa36b0c, 0xf5cf83,
            ],
            Self::RoseQuartz => [
                0x3b2632, 0x301e29, 0x80284e, 0x94325d, 0xbe356e, 0xcd407b, 0xf6a5c9,
            ],
            Self::Ember => [
                0x3b2c24, 0x30221b, 0x823b1b, 0x964721, 0xb94e18, 0xbf521b, 0xffb58b,
            ],
        };
        let (muted, border) = match self {
            Self::Evergreen => (0xbfd0c5, 0x5e7c6b),
            Self::Lagoon => (0xbdd0d1, 0x5c7b7e),
            Self::Amber => (0xd4cbb8, 0x807052),
            Self::RoseQuartz => (0xd5c0cb, 0x856275),
            Self::Ember => (0xd6c5ba, 0x856b5c),
            _ => (0xcbbfd0, 0x73667b),
        };
        Palette {
            background: rgb(background),
            surface: rgb(surface),
            muted: rgb(muted),
            accent: rgb(accent),
            accent_hover: rgb(accent_hover),
            primary: rgb(primary),
            primary_hover: rgb(primary_hover),
            border: rgb(border),
            ring: rgb(ring),
        }
    }

    pub fn apply(self, ctx: &egui::Context) {
        // Every app palette uses dark visuals. Select that style explicitly
        // before customizing it so a later OS light-theme report cannot switch
        // startup rendering back to egui's unstyled light widgets.
        ctx.set_theme(egui::Theme::Dark);
        let p = self.palette();
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        let v = &mut style.visuals;
        v.panel_fill = p.background;
        v.window_fill = p.surface;
        v.extreme_bg_color = p.accent;
        v.faint_bg_color = p.surface;
        v.code_bg_color = p.surface;
        v.override_text_color = Some(Color32::WHITE);
        v.selection.bg_fill = p.primary;
        v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        v.hyperlink_color = p.ring;
        v.warn_fg_color = rgb(0xf2d28b);
        v.error_fg_color = rgb(0xffaaaa);
        v.window_rounding = egui::Rounding::same(16.0);
        v.menu_rounding = egui::Rounding::same(12.0);
        v.window_stroke = Stroke::new(1.0_f32, p.border);
        v.interact_cursor = Some(egui::CursorIcon::PointingHand);
        for widget in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            widget.rounding = egui::Rounding::same(99.0);
            widget.fg_stroke = Stroke::new(1.5_f32, Color32::WHITE);
            widget.expansion = 0.0;
        }
        v.widgets.inactive.bg_fill = p.accent;
        v.widgets.inactive.weak_bg_fill = p.accent;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.hovered.bg_fill = p.accent_hover;
        v.widgets.hovered.weak_bg_fill = p.accent_hover;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.ring);
        v.widgets.active.bg_fill = p.primary_hover;
        v.widgets.active.weak_bg_fill = p.primary_hover;
        v.widgets.active.bg_stroke = Stroke::new(1.0_f32, p.ring);
        v.widgets.open.bg_fill = p.accent_hover;
        v.widgets.open.weak_bg_fill = p.accent_hover;
        v.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.ring);
        v.widgets.noninteractive.bg_fill = p.surface;
        v.widgets.noninteractive.weak_bg_fill = p.surface;
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
        v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.muted);
        style.spacing.item_spacing = egui::vec2(12.0, 14.0);
        style.spacing.button_padding = egui::vec2(20.0, 9.0);
        style.spacing.interact_size = egui::vec2(40.0, 32.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(23.0));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, FontId::monospace(12.0));
        style.animation_time = 0.15;
        ctx.set_style(style);
    }
}

pub fn button(
    ui: &mut egui::Ui,
    label: &str,
    size: [f32; 2],
    primary: bool,
    font_size: f32,
) -> egui::Response {
    // Reserve the shadow behind the native button, retaining keyboard activation.
    let shadow_slot = ui.painter().add(egui::Shape::Noop);
    let response = ui
        .scope(|ui| {
            if primary {
                let primary = ui.visuals().selection.bg_fill;
                let hover = ui.visuals().hyperlink_color;
                let primary_hover = ui.visuals().widgets.active.bg_fill;
                let v = ui.visuals_mut();
                v.widgets.inactive.weak_bg_fill = primary;
                v.widgets.hovered.weak_bg_fill = primary_hover;
                v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, hover);
            }
            ui.add_sized(
                size,
                egui::Button::new(RichText::new(label).size(font_size)),
            )
        })
        .inner;
    if ui.is_rect_visible(response.rect) {
        let pressed = response.is_pointer_button_down_on();
        let shadow = egui::epaint::Shadow {
            offset: egui::vec2(
                0.0,
                if pressed {
                    1.0
                } else if response.hovered() {
                    5.0
                } else {
                    4.0
                },
            ),
            blur: if response.hovered() { 5.0 } else { 3.0 },
            spread: 0.0,
            color: Color32::from_black_alpha(if ui.is_enabled() { 145 } else { 65 }),
        };
        ui.painter()
            .set(shadow_slot, shadow.as_shape(response.rect, 99.0));
    }
    response
}

// Prefer the reference UI's font when available, retaining egui's bundled
// fallbacks so both Windows and Linux installations work without extra assets.
pub fn install_fonts(ctx: &egui::Context) {
    let mut candidates = Vec::new();
    if let Some(windows) = std::env::var_os("WINDIR") {
        let fonts = std::path::PathBuf::from(windows).join("Fonts");
        candidates.extend([
            fonts.join("segoeui.ttf"),
            fonts.join("tahoma.ttf"),
            fonts.join("arial.ttf"),
        ]);
    }
    candidates.extend(
        [
            "/usr/share/fonts/TTF/segoeui.ttf",
            "/usr/share/fonts/truetype/msttcorefonts/Arial.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
        .map(std::path::PathBuf::from),
    );
    for path in candidates {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("interface".into(), egui::FontData::from_owned(bytes));
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "interface".into());
        ctx.set_fonts(fonts);
        break;
    }
}
