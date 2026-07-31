//! User-supplied background image, including animated GIFs.
//!
//! Stills are decoded once and uploaded as a single egui texture. GIFs are
//! decoded to a vector of frames, each uploaded as its own texture, and
//! advanced on a wall-clock timer.
//!
//! ## Why GIFs need a memory budget
//!
//! A GIF is stored compressed but must be uploaded decompressed: a 1920×1080
//! clip at 120 frames is roughly a gigabyte of RGBA, and a "wallpaper GIF" off
//! the internet can plausibly be exactly that. So frames are downscaled to
//! [`MAX_DIM_ANIMATED`] and collected only while they fit inside
//! [`FRAME_BUDGET_BYTES`]; past that the animation is truncated and loops
//! early. A background decoration must never be able to take the app down.

use eframe::egui::{self, Color32, Rect, TextureHandle, TextureOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How the image is mapped onto the window.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Fill the window, cropping whatever overflows. Preserves aspect ratio.
    Cover,
    /// Fit entirely inside the window, letterboxing. Preserves aspect ratio.
    Contain,
    /// Fill the window exactly, distorting the image.
    Stretch,
}

impl Fit {
    pub const ALL: [Fit; 3] = [Fit::Cover, Fit::Contain, Fit::Stretch];

    pub fn from_id(s: &str) -> Self {
        match s {
            "contain" => Fit::Contain,
            "stretch" => Fit::Stretch,
            _ => Fit::Cover,
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Fit::Cover => "cover",
            Fit::Contain => "contain",
            Fit::Stretch => "stretch",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Fit::Cover => "Fill (crop)",
            Fit::Contain => "Fit (letterbox)",
            Fit::Stretch => "Stretch",
        }
    }
}

/// Largest dimension uploaded for a still. Bigger images are downscaled first —
/// some GPUs reject textures beyond this outright.
const MAX_DIM: u32 = 3840;
/// Animated frames are capped lower, since there are many of them — but not
/// so low that a 1080p GIF gets resampled for no reason. Every resample costs
/// sharpness, and the byte budget below is the real memory guard.
const MAX_DIM_ANIMATED: u32 = 2560;
/// Total decoded RGBA budget for one animation (~160 MB).
const FRAME_BUDGET_BYTES: usize = 160 * 1024 * 1024;
/// Hard ceiling on frame count regardless of size.
const MAX_FRAMES: usize = 300;
/// GIFs may specify a zero delay; browsers clamp these, and so do we.
const MIN_FRAME_DELAY: f32 = 0.02;
const DEFAULT_FRAME_DELAY: f32 = 0.1;

struct Frame {
    texture: TextureHandle,
    /// Seconds this frame is shown for.
    delay: f32,
}

/// A decoded still or animation, plus its playback clock.
///
/// Shared by the wallpaper and the corner overlay — both need identical GIF
/// handling, and duplicating the frame-advance logic would guarantee the two
/// drift apart.
#[derive(Default)]
pub struct Animation {
    frames: Vec<Frame>,
    cursor: usize,
    accum: f32,
}

impl Animation {
    /// Decode from raw encoded bytes. `name` disambiguates the uploaded
    /// textures so the wallpaper and overlay don't collide in egui's cache.
    pub fn from_bytes(ctx: &egui::Context, bytes: &[u8], name: &str) -> Result<Self, String> {
        // Truncation is already logged by `collect_frames`; callers that need
        // to surface it in the UI (the wallpaper) track it themselves, so it
        // isn't carried on the animation.
        let (frames, _truncated) = decode_bytes(ctx, bytes, name)?;
        Ok(Self {
            frames,
            cursor: 0,
            accum: 0.0,
        })
    }

    /// Step the animation clock, skipping as many frames as `dt` covers.
    pub fn advance(&mut self, dt: f32) {
        if self.frames.len() < 2 {
            return;
        }
        self.accum += dt;
        // Bounded: a huge dt must not spin through thousands of frames.
        for _ in 0..self.frames.len() {
            let current = self.frames[self.cursor].delay;
            if self.accum < current {
                break;
            }
            self.accum -= current;
            self.cursor = (self.cursor + 1) % self.frames.len();
        }
    }

    pub fn current(&self) -> Option<&TextureHandle> {
        self.frames.get(self.cursor).map(|f| &f.texture)
    }

    /// How long until the next frame is due, or `None` for a still image.
    pub fn next_frame_in(&self) -> Option<Duration> {
        if self.frames.len() < 2 {
            return None;
        }
        let remaining = (self.frames[self.cursor].delay - self.accum).max(0.0);
        Some(Duration::from_secs_f32(remaining))
    }
}

#[derive(Default)]
pub struct BackgroundImage {
    frames: Vec<Frame>,
    loaded_from: Option<PathBuf>,
    /// A path we already failed to load. Remembered so a broken or deleted
    /// file doesn't retry (and log) on every single frame.
    failed: Option<PathBuf>,
    /// True when the animation was cut short by the memory budget.
    truncated: bool,
    cursor: usize,
    /// Seconds spent on the current frame.
    accum: f32,
}

impl BackgroundImage {
    /// Bring the cached texture(s) in line with the configured path and
    /// advance the animation. Returns the frame to paint, if any.
    pub fn sync(&mut self, ctx: &egui::Context, path: Option<&Path>) -> Option<&TextureHandle> {
        match path {
            None => {
                if self.loaded_from.is_some() || self.failed.is_some() {
                    self.clear();
                }
            }
            Some(p) => {
                let already_loaded = self.loaded_from.as_deref() == Some(p);
                let known_bad = self.failed.as_deref() == Some(p);
                if !already_loaded && !known_bad {
                    match decode(ctx, p) {
                        Ok((frames, truncated)) => {
                            self.clear();
                            self.frames = frames;
                            self.truncated = truncated;
                            self.loaded_from = Some(p.to_path_buf());
                        }
                        Err(e) => {
                            tracing::warn!("Background image {} failed to load: {e}", p.display());
                            self.clear();
                            self.failed = Some(p.to_path_buf());
                        }
                    }
                }
            }
        }

        self.advance(ctx.input(|i| i.stable_dt).min(0.25));
        self.frames.get(self.cursor).map(|f| &f.texture)
    }

    fn clear(&mut self) {
        self.frames.clear();
        self.loaded_from = None;
        self.failed = None;
        self.truncated = false;
        self.cursor = 0;
        self.accum = 0.0;
    }

    /// Step the animation clock, skipping as many frames as `dt` covers so a
    /// slow frame never desynchronises playback.
    fn advance(&mut self, dt: f32) {
        if self.frames.len() < 2 {
            return;
        }
        self.accum += dt;
        // Bounded: a huge dt must not spin through thousands of frames.
        for _ in 0..self.frames.len() {
            let current = self.frames[self.cursor].delay;
            if self.accum < current {
                break;
            }
            self.accum -= current;
            self.cursor = (self.cursor + 1) % self.frames.len();
        }
    }

    /// How long until the next frame is due, or `None` for a still image.
    ///
    /// The caller schedules a repaint with this rather than running the whole
    /// UI at full framerate for a 10 fps GIF.
    pub fn next_frame_in(&self) -> Option<Duration> {
        if self.frames.len() < 2 {
            return None;
        }
        let remaining = (self.frames[self.cursor].delay - self.accum).max(0.0);
        Some(Duration::from_secs_f32(remaining))
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn was_truncated(&self) -> bool {
        self.truncated
    }

    /// True when the configured path was tried and could not be decoded, so
    /// the settings panel can say so instead of silently showing nothing.
    pub fn is_broken(&self, path: Option<&Path>) -> bool {
        match (path, self.failed.as_deref()) {
            (Some(p), Some(f)) => p == f,
            _ => false,
        }
    }
}

/// Returns `(frames, truncated)`.
fn decode(ctx: &egui::Context, path: &Path) -> Result<(Vec<Frame>, bool), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    decode_bytes(ctx, &bytes, "rm_user_background")
}

fn decode_bytes(
    ctx: &egui::Context,
    bytes: &[u8],
    name: &str,
) -> Result<(Vec<Frame>, bool), String> {
    use image::AnimationDecoder;

    // Sniff the magic bytes rather than trusting the extension — people rename
    // files, and a mislabelled GIF should still animate.
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes))
            .map_err(|e| e.to_string())?;
        collect_frames(ctx, decoder.into_frames(), name)
    } else if is_webp(bytes) {
        // WebP is the awkward one: the same container holds stills and
        // animations, so ask the decoder which it is rather than guessing.
        let decoder = image::codecs::webp::WebPDecoder::new(std::io::Cursor::new(bytes))
            .map_err(|e| e.to_string())?;
        if decoder.has_animation() {
            collect_frames(ctx, decoder.into_frames(), name)
        } else {
            Ok((vec![decode_still(ctx, bytes, name)?], false))
        }
    } else {
        Ok((vec![decode_still(ctx, bytes, name)?], false))
    }
}

/// WebP files are RIFF containers: `RIFF` at offset 0, `WEBP` at offset 8.
fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

fn decode_still(ctx: &egui::Context, bytes: &[u8], name: &str) -> Result<Frame, String> {
    let mut img = image::load_from_memory(bytes).map_err(|e| e.to_string())?;
    if img.width() > MAX_DIM || img.height() > MAX_DIM {
        img = img.resize(MAX_DIM, MAX_DIM, image::imageops::FilterType::Lanczos3);
    }
    let rgba = img.to_rgba8();
    Ok(Frame {
        texture: upload(ctx, &rgba, name, 0),
        delay: DEFAULT_FRAME_DELAY,
    })
}

/// Shared frame-collection loop for every animated format.
///
/// Downscaling and the memory budget live here rather than per-codec, so GIF
/// and WebP cannot drift into behaving differently.
fn collect_frames(
    ctx: &egui::Context,
    frames: image::Frames<'_>,
    name: &str,
) -> Result<(Vec<Frame>, bool), String> {
    let decoder = frames;

    let mut out: Vec<Frame> = Vec::new();
    let mut used_bytes = 0usize;
    let mut truncated = false;

    for (i, frame) in decoder.enumerate() {
        if i >= MAX_FRAMES {
            truncated = true;
            break;
        }
        let frame = frame.map_err(|e| e.to_string())?;

        // Delay is a rational in milliseconds; a zero denominator would be
        // malformed, so fall back rather than dividing by it.
        let (numer, denom) = frame.delay().numer_denom_ms();
        let delay = if denom == 0 {
            DEFAULT_FRAME_DELAY
        } else {
            (numer as f32 / denom as f32 / 1000.0).max(MIN_FRAME_DELAY)
        };

        let buf = frame.into_buffer();
        let (w, h) = (buf.width(), buf.height());
        let rgba = if w > MAX_DIM_ANIMATED || h > MAX_DIM_ANIMATED {
            image::DynamicImage::ImageRgba8(buf)
                .resize(
                    MAX_DIM_ANIMATED,
                    MAX_DIM_ANIMATED,
                    image::imageops::FilterType::Lanczos3,
                )
                .to_rgba8()
        } else {
            buf
        };

        let cost = rgba.as_raw().len();
        if used_bytes + cost > FRAME_BUDGET_BYTES && !out.is_empty() {
            truncated = true;
            break;
        }
        used_bytes += cost;

        out.push(Frame {
            texture: upload(ctx, &rgba, name, i),
            delay,
        });
    }

    if out.is_empty() {
        return Err("the animation contained no decodable frames".to_string());
    }
    if truncated {
        tracing::warn!(
            "Background animation truncated to {} frame(s) (~{} MB) to stay within the memory budget",
            out.len(),
            used_bytes / (1024 * 1024)
        );
    }
    Ok((out, truncated))
}

fn upload(
    ctx: &egui::Context,
    rgba: &image::RgbaImage,
    name: &str,
    index: usize,
) -> TextureHandle {
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    // `TextureOptions::LINEAR` leaves `mipmap_mode` at `None`, so every
    // minification is a single raw bilinear sample. A wallpaper is almost
    // always being drawn smaller than its source, which is precisely the case
    // mipmaps exist for — without them the image reads soft and slightly
    // aliased next to crisp UI text.
    let options = TextureOptions {
        mipmap_mode: Some(egui::TextureFilter::Linear),
        ..TextureOptions::LINEAR
    };
    ctx.load_texture(format!("{name}_{index}"), color, options)
}

/// Paint the image across `screen` honouring the fit mode and opacity.
///
/// Opacity is applied as a white tint with alpha rather than by rebuilding the
/// texture, so the slider is free to drag.
pub fn paint(painter: &egui::Painter, screen: Rect, tex: &TextureHandle, opacity: f32, fit: Fit) {
    let tint = Color32::from_white_alpha((opacity.clamp(0.0, 1.0) * 255.0) as u8);
    let full_uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    let img = tex.size_vec2();
    if img.x <= 0.0 || img.y <= 0.0 || screen.width() <= 0.0 || screen.height() <= 0.0 {
        return;
    }

    match fit {
        Fit::Stretch => {
            painter.image(tex.id(), screen, full_uv, tint);
        }
        Fit::Cover => {
            // Scale so the image covers both axes, then express the visible
            // portion as a centred UV sub-rectangle.
            let scale = (screen.width() / img.x).max(screen.height() / img.y);
            let visible_u = (screen.width() / scale / img.x).min(1.0);
            let visible_v = (screen.height() / scale / img.y).min(1.0);
            let u0 = (1.0 - visible_u) * 0.5;
            let v0 = (1.0 - visible_v) * 0.5;
            let uv = Rect::from_min_max(
                egui::pos2(u0, v0),
                egui::pos2(u0 + visible_u, v0 + visible_v),
            );
            painter.image(tex.id(), screen, uv, tint);
        }
        Fit::Contain => {
            let scale = (screen.width() / img.x).min(screen.height() / img.y);
            let size = egui::vec2(img.x * scale, img.y * scale);
            let rect = Rect::from_center_size(screen.center(), size);
            painter.image(tex.id(), rect, full_uv, tint);
        }
    }
}
