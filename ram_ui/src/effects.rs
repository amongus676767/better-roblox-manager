//! Animated background effects painted beneath the whole UI.
//!
//! Everything here draws onto egui's [`Order::Background`] layer, which sits
//! under every panel and window. It works because [`crate::theme`] gives the
//! panels a translucent fill — with opaque panels this would all be invisible.
//!
//! ## Cost
//!
//! Any enabled effect forces continuous repainting (the app is otherwise in
//! eframe's reactive mode and sleeps between interactions). That is the whole
//! price of animation: roughly a few percent CPU and a wake-up every frame
//! instead of every two seconds. `effects_enabled = false` restores the
//! original sleepy behaviour exactly, which is the right default on a laptop.

use eframe::egui::{self, Color32, Pos2, Rect};
use std::collections::VecDeque;

use crate::theme::Palette;

/// The user's background image, if any, plus how to draw it.
pub struct BackgroundCfg<'a> {
    pub texture: Option<&'a egui::TextureHandle>,
    pub opacity: f32,
    pub fit: crate::background::Fit,
    /// Black scrim alpha drawn over the backdrop, 0.0..=1.0.
    pub dim: f32,
}

/// Which effects are switched on, and how strongly.
#[derive(Clone, Copy)]
pub struct EffectSettings {
    pub enabled: bool,
    pub nebula: bool,
    pub stars: bool,
    pub rain: bool,
    pub cursor_glow: bool,
    /// 0.0..=1.0 — scales particle counts and opacity, not speed.
    pub intensity: f32,
}

impl EffectSettings {
    /// True when at least one effect will actually draw something.
    fn any_active(&self) -> bool {
        self.enabled && (self.nebula || self.stars || self.rain || self.cursor_glow)
    }
}

#[derive(Clone, Copy)]
struct Drop {
    x: f32,
    y: f32,
    speed: f32,
    len: f32,
    alpha: f32,
}

/// Per-frame mutable state: raindrop positions and the cursor trail.
///
/// Stars and nebula are *not* stored — they're derived from a hash of their
/// index plus the clock, so they stay put across resizes and cost no memory.
pub struct EffectState {
    drops: Vec<Drop>,
    trail: VecDeque<Pos2>,
    seeded_for: f32,
    rng: u32,
}

impl Default for EffectState {
    fn default() -> Self {
        Self {
            drops: Vec::new(),
            trail: VecDeque::new(),
            seeded_for: 0.0,
            rng: 0x9E37_79B9,
        }
    }
}

impl EffectState {
    /// xorshift32 — deterministic, fast, and good enough for scattering rain.
    fn next_f32(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / 16_777_216.0
    }
}

/// Stable pseudo-random value in 0..1 derived from an integer seed.
/// Used for star placement so the sky doesn't reshuffle every frame.
fn hash01(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(0x27D4_EB2D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x85EB_CA6B);
    x ^= x >> 13;
    (x >> 8) as f32 / 16_777_216.0
}

fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Paint the full background for this frame.
///
/// Returns `true` if anything animated was drawn, meaning the caller must keep
/// requesting repaints.
pub fn render(
    ctx: &egui::Context,
    state: &mut EffectState,
    palette: &Palette,
    cfg: EffectSettings,
    bg: BackgroundCfg<'_>,
) -> bool {
    let screen = ctx.screen_rect();
    // Must be the *same* layer the panels use — not a new layer that merely
    // shares `Order::Background`. egui's `GraphicLayers::drain` emits layers
    // registered in `area_order` first and any unregistered layer afterwards,
    // so a separate background-order layer is painted ON TOP of the whole UI.
    // Sharing the panel layer and painting before any panel is added puts
    // these shapes first in the same list, i.e. genuinely underneath.
    let painter = ctx.layer_painter(egui::LayerId::background());

    // The base gradient is drawn even with effects off, so a theme still
    // changes the look of the app when someone wants zero animation. A user
    // image sits on top of it, and the animated layers on top of that.
    paint_gradient(&painter, screen, palette);
    if let Some(tex) = bg.texture {
        crate::background::paint(&painter, screen, tex, bg.opacity, bg.fit);
    }

    // Scrim sits between the wallpaper and the animated layers: it exists to
    // push the backdrop away from the UI, not to dull the effects drawn for
    // the user's benefit.
    if bg.dim > 0.0 {
        painter.rect_filled(
            screen,
            0.0,
            Color32::from_black_alpha((bg.dim.clamp(0.0, 1.0) * 255.0) as u8),
        );
    }

    if !cfg.any_active() {
        return false;
    }

    let t = ctx.input(|i| i.time) as f32;
    let dt = ctx.input(|i| i.stable_dt).min(0.1);
    let k = cfg.intensity.clamp(0.0, 1.0);

    if cfg.nebula {
        paint_nebula(&painter, screen, palette, t, k);
    }
    if cfg.stars {
        paint_stars(&painter, screen, palette, t, k);
    }
    if cfg.rain {
        paint_rain(&painter, state, screen, palette, dt, k);
    }
    if cfg.cursor_glow {
        let pos = ctx.input(|i| i.pointer.latest_pos());
        paint_cursor(&painter, state, palette, pos, k);
    } else {
        state.trail.clear();
    }

    true
}

/// Vertical two-stop gradient across the whole window, as a single quad mesh
/// with per-vertex colours. Cheaper and smoother than stacking rects.
fn paint_gradient(painter: &egui::Painter, screen: Rect, p: &Palette) {
    let mut mesh = egui::Mesh::default();
    let uv = egui::epaint::WHITE_UV;
    for (corner, color) in [
        (screen.left_top(), p.space_top),
        (screen.right_top(), p.space_top),
        (screen.left_bottom(), p.space_bottom),
        (screen.right_bottom(), p.space_bottom),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: corner,
            uv,
            color,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 2, 1, 3]);
    painter.add(egui::Shape::mesh(mesh));
}

/// Soft drifting nebula clouds.
///
/// egui has no radial gradient primitive, so each cloud is a stack of
/// concentric circles whose alpha falls off toward the edge. Twelve rings is
/// the point where the banding stops being visible at typical window sizes.
fn paint_nebula(painter: &egui::Painter, screen: Rect, p: &Palette, t: f32, k: f32) {
    const CLOUDS: u32 = 5;
    const RINGS: u32 = 12;

    for i in 0..CLOUDS {
        let seed = i * 977;
        let base_x = hash01(seed) * screen.width();
        let base_y = hash01(seed + 1) * screen.height();
        // Each cloud drifts on its own slow lissajous path.
        let drift = 40.0 + hash01(seed + 2) * 60.0;
        let sx = (t * (0.05 + hash01(seed + 3) * 0.04) + i as f32).sin() * drift;
        let sy = (t * (0.04 + hash01(seed + 4) * 0.03) + i as f32 * 1.7).cos() * drift;
        let center = Pos2::new(screen.left() + base_x + sx, screen.top() + base_y + sy);

        let radius = (140.0 + hash01(seed + 5) * 220.0) * (0.6 + 0.4 * k);
        let tint = if i % 2 == 0 { p.nebula_a } else { p.nebula_b };
        // Gentle breathing so the clouds never look frozen.
        let pulse = 0.85 + 0.15 * (t * 0.3 + i as f32).sin();

        for r in 0..RINGS {
            let f = r as f32 / RINGS as f32;
            let ring_radius = radius * (1.0 - f) * pulse;
            let alpha = 0.020 * k * (1.0 - f);
            painter.circle_filled(center, ring_radius, with_alpha(tint, alpha));
        }
    }
}

/// Twinkling starfield. Positions come from `hash01` so they are identical
/// every frame; only brightness animates.
fn paint_stars(painter: &egui::Painter, screen: Rect, p: &Palette, t: f32, k: f32) {
    let count = (60.0 + 200.0 * k) as u32;
    for i in 0..count {
        let seed = i * 7919;
        let x = screen.left() + hash01(seed) * screen.width();
        let y = screen.top() + hash01(seed + 1) * screen.height();
        let size = 0.6 + hash01(seed + 2) * 1.6;
        // Per-star phase and rate so they don't blink in unison.
        let phase = hash01(seed + 3) * std::f32::consts::TAU;
        let rate = 0.4 + hash01(seed + 4) * 1.4;
        let twinkle = 0.35 + 0.65 * (0.5 + 0.5 * (t * rate + phase).sin());
        painter.circle_filled(
            Pos2::new(x, y),
            size,
            with_alpha(p.star, twinkle * 0.75 * k.max(0.25)),
        );
    }
}

/// Falling rain streaks drawn as short vertical lines with a slight slant.
fn paint_rain(
    painter: &egui::Painter,
    state: &mut EffectState,
    screen: Rect,
    p: &Palette,
    dt: f32,
    k: f32,
) {
    let target = (30.0 + 130.0 * k) as usize;

    // Re-seed on first use or when the window is resized enough to matter,
    // so drops always cover the current viewport.
    if state.drops.len() != target || (state.seeded_for - screen.width()).abs() > 1.0 {
        state.seeded_for = screen.width();
        state.drops.clear();
        for _ in 0..target {
            let x = state.next_f32();
            let y = state.next_f32();
            let s = state.next_f32();
            let l = state.next_f32();
            let a = state.next_f32();
            state.drops.push(Drop {
                x: screen.left() + x * screen.width(),
                y: screen.top() + y * screen.height(),
                speed: 90.0 + s * 260.0,
                len: 6.0 + l * 16.0,
                alpha: 0.15 + a * 0.35,
            });
        }
    }

    for d in &mut state.drops {
        d.y += d.speed * dt;
        // Slight horizontal drift makes it read as weather rather than a
        // scrolling texture.
        d.x += d.speed * dt * 0.12;
        if d.y > screen.bottom() {
            d.y = screen.top() - d.len;
        }
        if d.x > screen.right() {
            d.x = screen.left();
        }

        painter.line_segment(
            [
                Pos2::new(d.x, d.y),
                Pos2::new(d.x - d.len * 0.12, d.y - d.len),
            ],
            egui::Stroke::new(1.0_f32, with_alpha(p.rain, d.alpha * k)),
        );
    }
}

/// Ethereal glow that follows the cursor, plus a fading comet trail behind it.
fn paint_cursor(
    painter: &egui::Painter,
    state: &mut EffectState,
    p: &Palette,
    pos: Option<Pos2>,
    k: f32,
) {
    const TRAIL_MAX: usize = 36;

    if let Some(pos) = pos {
        // Only record real movement, otherwise a stationary cursor collapses
        // the whole trail into one bright dot.
        let moved = state
            .trail
            .back()
            .map_or(true, |last| last.distance(pos) > 2.0);
        if moved {
            state.trail.push_back(pos);
        }
    }
    while state.trail.len() > TRAIL_MAX {
        state.trail.pop_front();
    }

    // Trail: older points are smaller and fainter.
    let n = state.trail.len().max(1) as f32;
    for (i, point) in state.trail.iter().enumerate() {
        let age = i as f32 / n;
        painter.circle_filled(
            *point,
            1.5 + 5.0 * age,
            with_alpha(p.accent, 0.28 * age * age * k),
        );
    }

    // Glow: concentric rings approximating a radial falloff.
    if let Some(pos) = pos {
        const RINGS: u32 = 16;
        let radius = 70.0 * (0.5 + 0.5 * k);
        for r in 0..RINGS {
            let f = r as f32 / RINGS as f32;
            painter.circle_filled(
                pos,
                radius * (1.0 - f),
                with_alpha(p.accent, 0.022 * k * (1.0 - f)),
            );
        }
    }
}
