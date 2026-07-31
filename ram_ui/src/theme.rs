//! Colour themes for the app.
//!
//! A [`Palette`] drives two things: the egui widget [`Visuals`] (panels, text,
//! borders) and the animated background painted by [`crate::effects`]. Keeping
//! both in one struct is what stops the chrome and the starfield drifting out
//! of sync when a theme is switched at runtime.
//!
//! Panel fills are deliberately **translucent** — that is what lets the cosmos
//! show through behind the account list. Alpha is kept high enough (≈215) that
//! body text stays readable over a moving starfield; dropping it much below
//! that trades legibility for prettiness and is not worth it.

use eframe::egui::{self, Color32};

#[derive(Clone, Copy)]
pub struct Palette {
    /// Stable identifier persisted in `config.json`.
    pub id: &'static str,
    /// Label shown in the settings dropdown.
    pub name: &'static str,
    /// Vertical gradient of the deep background.
    pub space_top: Color32,
    pub space_bottom: Color32,
    /// The two nebula cloud tints.
    pub nebula_a: Color32,
    pub nebula_b: Color32,
    /// Starfield colour.
    pub star: Color32,
    /// Cursor glow / trail, and the widget accent colour.
    pub accent: Color32,
    /// Falling rain streaks.
    pub rain: Color32,
    /// Base panel colour, opaque. Alpha comes from the user's panel-opacity
    /// setting at apply time, not from the palette — baking it in here is what
    /// made every theme paint an unremovable 84%-opaque film over the user's
    /// wallpaper.
    pub panel: Color32,
    /// Slightly darker base colour for inset section frames.
    pub inset: Color32,
    /// Primary text colour.
    pub text: Color32,
}

/// Every selectable theme, in dropdown order.
pub const THEMES: &[Palette] = &[
    Palette {
        id: "cosmos",
        name: "Cosmos",
        space_top: Color32::from_rgb(8, 10, 28),
        space_bottom: Color32::from_rgb(3, 4, 12),
        nebula_a: Color32::from_rgb(70, 110, 220),
        nebula_b: Color32::from_rgb(140, 90, 220),
        star: Color32::from_rgb(220, 232, 255),
        accent: Color32::from_rgb(120, 190, 255),
        rain: Color32::from_rgb(150, 200, 255),
        panel: Color32::from_rgb(14, 18, 40),
        inset: Color32::from_rgb(9, 12, 28),
        text: Color32::from_rgb(226, 234, 250),
    },
    Palette {
        id: "nebula",
        name: "Nebula",
        space_top: Color32::from_rgb(26, 8, 32),
        space_bottom: Color32::from_rgb(8, 3, 12),
        nebula_a: Color32::from_rgb(220, 80, 170),
        nebula_b: Color32::from_rgb(120, 60, 220),
        star: Color32::from_rgb(255, 226, 245),
        accent: Color32::from_rgb(240, 130, 210),
        rain: Color32::from_rgb(235, 160, 220),
        panel: Color32::from_rgb(32, 14, 40),
        inset: Color32::from_rgb(22, 9, 28),
        text: Color32::from_rgb(246, 228, 244),
    },
    Palette {
        id: "aurora",
        name: "Aurora",
        space_top: Color32::from_rgb(4, 26, 26),
        space_bottom: Color32::from_rgb(2, 10, 12),
        nebula_a: Color32::from_rgb(60, 220, 170),
        nebula_b: Color32::from_rgb(60, 140, 220),
        star: Color32::from_rgb(224, 255, 244),
        accent: Color32::from_rgb(90, 230, 190),
        rain: Color32::from_rgb(140, 240, 210),
        panel: Color32::from_rgb(10, 32, 32),
        inset: Color32::from_rgb(6, 22, 23),
        text: Color32::from_rgb(224, 246, 240),
    },
    Palette {
        id: "abyss",
        name: "Abyss",
        space_top: Color32::from_rgb(4, 14, 24),
        space_bottom: Color32::from_rgb(1, 4, 8),
        nebula_a: Color32::from_rgb(30, 90, 150),
        nebula_b: Color32::from_rgb(20, 50, 110),
        star: Color32::from_rgb(190, 214, 236),
        accent: Color32::from_rgb(80, 160, 220),
        rain: Color32::from_rgb(110, 170, 220),
        panel: Color32::from_rgb(8, 18, 30),
        inset: Color32::from_rgb(5, 12, 21),
        text: Color32::from_rgb(212, 226, 240),
    },
    Palette {
        id: "ember",
        name: "Ember",
        space_top: Color32::from_rgb(28, 12, 6),
        space_bottom: Color32::from_rgb(10, 4, 2),
        nebula_a: Color32::from_rgb(230, 120, 50),
        nebula_b: Color32::from_rgb(180, 50, 60),
        star: Color32::from_rgb(255, 236, 214),
        accent: Color32::from_rgb(250, 160, 90),
        rain: Color32::from_rgb(240, 170, 120),
        panel: Color32::from_rgb(32, 16, 10),
        inset: Color32::from_rgb(22, 10, 6),
        text: Color32::from_rgb(248, 232, 220),
    },
    Palette {
        id: "slate",
        name: "Slate (plain dark)",
        space_top: Color32::from_rgb(24, 24, 27),
        space_bottom: Color32::from_rgb(16, 16, 18),
        nebula_a: Color32::from_rgb(70, 70, 80),
        nebula_b: Color32::from_rgb(50, 50, 58),
        star: Color32::from_rgb(200, 200, 210),
        accent: Color32::from_rgb(110, 170, 230),
        rain: Color32::from_rgb(150, 150, 165),
        panel: Color32::from_rgb(30, 30, 34),
        inset: Color32::from_rgb(22, 22, 26),
        text: Color32::from_rgb(226, 226, 232),
    },
];

/// Look a palette up by its persisted id, falling back to the first theme.
pub fn by_id(id: &str) -> &'static Palette {
    THEMES.iter().find(|p| p.id == id).unwrap_or(&THEMES[0])
}

/// Apply a palette to egui's global visuals.
///
/// This supersedes the old `apply_global_style` in `main.rs`: it keeps that
/// function's widget-border tweaks (without them TextEdits are invisible
/// against their section frames) and layers the theme colours on top.
pub fn apply(ctx: &egui::Context, p: &Palette, panel_opacity: f32) {
    let a = (panel_opacity.clamp(0.0, 1.0) * 255.0) as u8;
    let translucent = |c: Color32, alpha: u8| {
        Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
    };
    // Inset frames sit a little more solid than the panels behind them so
    // input fields stay findable even at low opacity.
    let inset_a = a.saturating_add((255 - a) / 3);
    let panel = translucent(p.panel, a);
    let inset = translucent(p.inset, inset_a);
    ctx.style_mut(|style| {
        let v = &mut style.visuals;
        *v = egui::Visuals::dark();

        v.panel_fill = panel;
        v.window_fill = panel;
        v.extreme_bg_color = inset;
        v.faint_bg_color = inset;
        v.override_text_color = Some(p.text);
        v.hyperlink_color = p.accent;
        v.selection.bg_fill = p.accent.linear_multiply(0.35);
        v.selection.stroke = egui::Stroke::new(1.0_f32, p.accent);
        v.window_stroke = egui::Stroke::new(1.0_f32, p.accent.linear_multiply(0.4));

        // Widget borders — carried over from the original apply_global_style.
        let border = p.accent.linear_multiply(0.35);
        let border_hover = p.accent.linear_multiply(0.7);
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, border);
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, border_hover);
        v.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, p.accent);
        v.widgets.inactive.rounding = egui::Rounding::same(3.0);
        v.widgets.hovered.rounding = egui::Rounding::same(3.0);
        v.widgets.active.rounding = egui::Rounding::same(3.0);

        // Widget backgrounds need to stay translucent too, or buttons punch
        // opaque holes in the starfield.
        v.widgets.noninteractive.bg_fill = inset;
        v.widgets.inactive.bg_fill = inset;
        v.widgets.hovered.bg_fill = panel;
        v.widgets.active.bg_fill = panel;
    });
}
