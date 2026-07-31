//! Decorative corner overlay.
//!
//! Paints a fetched image into a corner of the window, above the UI. Unlike
//! the wallpaper this uses [`egui::Order::Foreground`], which sits above the
//! panel layer — the whole point is for it to be visible over the account list
//! rather than hidden behind it.
//!
//! Attribution: when the source API supplies an artist name, it is drawn under
//! the image. These are working illustrators' drawings and the app displaying
//! them should say whose they are.

use eframe::egui::{self, Color32, Rect};

use crate::background::Animation;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    BottomRight,
    BottomLeft,
    TopRight,
    TopLeft,
}

impl Corner {
    pub const ALL: [Corner; 4] = [
        Corner::BottomRight,
        Corner::BottomLeft,
        Corner::TopRight,
        Corner::TopLeft,
    ];

    pub fn from_id(s: &str) -> Self {
        match s {
            "bottom_left" => Corner::BottomLeft,
            "top_right" => Corner::TopRight,
            "top_left" => Corner::TopLeft,
            _ => Corner::BottomRight,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Corner::BottomRight => "bottom_right",
            Corner::BottomLeft => "bottom_left",
            Corner::TopRight => "top_right",
            Corner::TopLeft => "top_left",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Corner::BottomRight => "Bottom right",
            Corner::BottomLeft => "Bottom left",
            Corner::TopRight => "Top right",
            Corner::TopLeft => "Top left",
        }
    }
}

/// Margin between the image and the window edges.
const MARGIN: f32 = 12.0;
/// Vertical space reserved beneath the image for the credit line, so the text
/// sits below the artwork instead of on top of it.
const CREDIT_HEIGHT: f32 = 16.0;
const CREDIT_FONT_SIZE: f32 = 10.0;

#[derive(Default)]
pub struct Overlay {
    animation: Option<Animation>,
    artist: Option<String>,
    source_site: Option<String>,
    /// Set while a fetch is in flight, so the button can show progress and we
    /// don't queue up a pile of concurrent requests.
    pub loading: bool,
    /// Last failure, surfaced in settings rather than silently doing nothing.
    pub last_error: Option<String>,
}

impl Overlay {
    /// Replace the current image with freshly downloaded bytes.
    pub fn set_image(
        &mut self,
        ctx: &egui::Context,
        bytes: &[u8],
        artist: Option<String>,
        source_site: String,
    ) {
        match Animation::from_bytes(ctx, bytes, "rm_overlay") {
            Ok(anim) => {
                self.animation = Some(anim);
                self.artist = artist;
                self.source_site = Some(source_site);
                self.last_error = None;
            }
            Err(e) => {
                tracing::warn!("Overlay image could not be decoded: {e}");
                self.last_error = Some(format!("Could not decode the image: {e}"));
            }
        }
        self.loading = false;
    }

    pub fn fail(&mut self, message: String) {
        tracing::warn!("Overlay fetch failed: {message}");
        self.last_error = Some(message);
        self.loading = false;
    }

    pub fn clear(&mut self) {
        self.animation = None;
        self.artist = None;
        self.source_site = None;
        self.last_error = None;
    }

    pub fn has_image(&self) -> bool {
        self.animation.is_some()
    }

    /// Credit line for the settings panel, if the source gave us one.
    pub fn credit(&self) -> Option<String> {
        let site = self.source_site.as_deref()?;
        Some(match self.artist.as_deref() {
            Some(artist) => format!("Art by {artist} — via {site}"),
            None => format!("via {site}"),
        })
    }

    /// Draw the overlay. Returns how long until the next animation frame is
    /// due, so the caller can schedule a repaint instead of spinning.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        opacity: f32,
        height_fraction: f32,
        corner: Corner,
        show_credit: bool,
    ) -> Option<std::time::Duration> {
        // Resolved before the mutable borrow below: `animation.as_mut()` is
        // held until the end of this function, so `self.credit()` cannot take
        // an immutable borrow of `self` in the middle of it.
        let credit = if show_credit { self.credit() } else { None };

        let anim = self.animation.as_mut()?;
        anim.advance(ctx.input(|i| i.stable_dt).min(0.25));
        let tex = anim.current()?;

        let screen = ctx.screen_rect();
        let img = tex.size_vec2();
        if img.x <= 0.0 || img.y <= 0.0 {
            return None;
        }

        // Height drives the size; width follows from the aspect ratio, then
        // both are clamped so a very wide image can't span the whole window.
        let target_h = (screen.height() * height_fraction.clamp(0.05, 0.9)).max(24.0);
        let mut size = egui::vec2(target_h * (img.x / img.y), target_h);
        let max_w = screen.width() * 0.6;
        if size.x > max_w {
            size = egui::vec2(max_w, max_w * (img.y / img.x));
        }

        // Bottom-anchored corners lift the image by the credit's height, so the
        // text has somewhere to live. Without this the credit is clamped inside
        // the window and ends up printed over the bottom of the artwork.
        let reserved = if credit.is_some() { CREDIT_HEIGHT } else { 0.0 };

        let rect = match corner {
            Corner::BottomRight => Rect::from_min_size(
                egui::pos2(
                    screen.right() - size.x - MARGIN,
                    screen.bottom() - size.y - MARGIN - reserved,
                ),
                size,
            ),
            Corner::BottomLeft => Rect::from_min_size(
                egui::pos2(
                    screen.left() + MARGIN,
                    screen.bottom() - size.y - MARGIN - reserved,
                ),
                size,
            ),
            Corner::TopRight => Rect::from_min_size(
                egui::pos2(screen.right() - size.x - MARGIN, screen.top() + MARGIN),
                size,
            ),
            Corner::TopLeft => {
                Rect::from_min_size(egui::pos2(screen.left() + MARGIN, screen.top() + MARGIN), size)
            }
        };

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("rm_corner_overlay"),
        ));
        painter.image(
            tex.id(),
            rect,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::from_white_alpha((opacity.clamp(0.0, 1.0) * 255.0) as u8),
        );

        if let Some(credit) = credit {
            let alpha = opacity.clamp(0.0, 1.0);
            // Lay the text out first so the backing plate can be sized to it.
            // Artist names are frequently CJK and unpredictably wide, so
            // guessing a width here would clip them.
            let galley = painter.layout_no_wrap(
                credit,
                egui::FontId::proportional(CREDIT_FONT_SIZE),
                Color32::from_white_alpha((alpha * 210.0) as u8),
            );

            let pad = egui::vec2(5.0, 2.0);
            let plate_size = galley.size() + pad * 2.0;
            // Centred under the image, then nudged back inside the window so a
            // name wider than the artwork can't run off the edge.
            let mut plate_min = egui::pos2(
                rect.center().x - plate_size.x * 0.5,
                rect.bottom() + 2.0,
            );
            plate_min.x = plate_min
                .x
                .clamp(screen.left() + 2.0, (screen.right() - plate_size.x - 2.0).max(screen.left() + 2.0));
            let plate = Rect::from_min_size(plate_min, plate_size);

            // A dark plate keeps the credit readable over a light image; white
            // text alone disappears against pale artwork.
            painter.rect_filled(
                plate,
                3.0,
                Color32::from_black_alpha((alpha * 130.0) as u8),
            );
            painter.galley(plate.min + pad, galley, Color32::WHITE);
        }

        anim.next_frame_in()
    }
}
