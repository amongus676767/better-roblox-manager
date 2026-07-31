#![windows_subsystem = "windows"]

mod app;
mod audio;
mod background;
mod bridge;
mod browser_login;
mod components;
mod effects;
mod overlay;
mod theme;
mod toast;

use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

/// Canonical data directory: `%APPDATA%\RM`.
pub fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("RM")
}

/// One-time migration: turn each entry of `config.favorite_places` into a
/// standalone preset file under `presets/`, then clear the list in config.
/// Runs silently and is a no-op when there's nothing to migrate.
fn maybe_migrate_favorites(data_dir: &std::path::Path) {
    let config_path = data_dir.join("config.json");
    if !config_path.is_file() {
        return;
    }
    let mut config = ram_core::AppConfig::load(&config_path);
    if config.favorite_places.is_empty() {
        return;
    }
    let mut migrated = 0;
    for fav in config.favorite_places.drain(..) {
        let preset = ram_core::models::LaunchPreset {
            name: fav.name,
            place_id: fav.place_id,
            job_id: None,
        };
        if ram_core::presets::save(data_dir, &preset, None).is_ok() {
            migrated += 1;
        }
    }
    if migrated > 0 {
        let _ = config.save(&config_path);
        tracing::info!("Migrated {migrated} legacy favorite(s) into preset files");
    }
}

/// Check for legacy data files next to the exe and offer to migrate them.
fn maybe_migrate_legacy_data(data_dir: &std::path::Path) {
    let legacy_config = PathBuf::from("config.json");
    let legacy_accounts = PathBuf::from("accounts.dat");

    let has_legacy = legacy_config.is_file() || legacy_accounts.is_file();
    let has_new = data_dir.join("config.json").is_file();

    if !has_legacy || has_new {
        return;
    }

    // Show a native dialog before the egui window opens
    let result = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Info)
        .set_title("RM - Migrate Data")
        .set_description(
            "RM now stores data in a standard location so it works \
             no matter where the exe is placed.\n\n\
             Found existing data next to the exe. Move it to the new location?\n\n\
             • Yes: move files (recommended)\n\
             • No: keep using files next to the exe",
        )
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();

    if result == rfd::MessageDialogResult::Yes {
        if let Err(e) = std::fs::create_dir_all(data_dir) {
            tracing::error!("Failed to create data dir: {e}");
            return;
        }
        for name in &["config.json", "accounts.dat"] {
            let src = PathBuf::from(name);
            if src.is_file() {
                let dst = data_dir.join(name);
                if let Err(e) = std::fs::rename(&src, &dst) {
                    // rename can fail across volumes; fall back to copy+delete
                    if let Err(e2) = std::fs::copy(&src, &dst) {
                        tracing::error!("Failed to migrate {name}: rename={e}, copy={e2}");
                    } else {
                        let _ = std::fs::remove_file(&src);
                    }
                }
            }
        }
    }
}

fn main() {
    let data_dir = data_dir();
    let _ = std::fs::create_dir_all(&data_dir);

    // Log to a file so crashes are visible even without a console
    // (the #[windows_subsystem = "windows"] attribute suppresses stderr).
    let log_path = data_dir.join("rm.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .ok();

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        );

    if let Some(file) = log_file {
        subscriber.with_writer(std::sync::Mutex::new(file)).init();
    } else {
        subscriber.init();
    }

    // Install a panic hook that flushes the message to the log before dying.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("PANIC: {info}");
        prev_hook(info);
    }));

    // Browser-login child mode: re-entry point when the parent UI spawns us
    // with the browser_login flag. Hosts the webview on this process's main
    // thread and exits when the cookie is captured or the user cancels.
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 4 && args[1] == browser_login::FLAG {
        let profile_dir = PathBuf::from(&args[2]);
        let outfile = PathBuf::from(&args[3]);
        let code = browser_login::run_child(profile_dir, outfile);
        std::process::exit(code);
    }
    // "Open browser as" child mode — same re-exec trick, but pre-loaded with
    // an account's cookie and left open until the user closes the window.
    if args.len() >= 4 && args[1] == browser_login::BROWSE_AS_FLAG {
        let profile_dir = PathBuf::from(&args[2]);
        let cookie_in = PathBuf::from(&args[3]);
        let label = args.get(4).cloned().unwrap_or_default();
        let code = browser_login::run_browse_as_child(profile_dir, cookie_in, label);
        std::process::exit(code);
    }

    // Offer to migrate legacy data from the exe directory
    maybe_migrate_legacy_data(&data_dir);

    // Migrate legacy `favorite_places` (inline in config.json) into the new
    // per-file preset system. Runs once: when the migration succeeds we clear
    // the config field so subsequent startups skip the loop.
    maybe_migrate_favorites(&data_dir);

    // Resolve config and account paths.
    // If a legacy config.json still exists next to the exe (user declined migration),
    // keep using local paths for backwards compatibility.
    let (config_path, config) = if PathBuf::from("config.json").is_file()
        && !data_dir.join("config.json").is_file()
    {
        // User declined migration — use local files
        let p = PathBuf::from("config.json");
        let c = ram_core::AppConfig::load(&p);
        (p, c)
    } else {
        let p = data_dir.join("config.json");
        let mut c = ram_core::AppConfig::load(&p);
        // Ensure accounts_path is absolute under the data dir
        if c.accounts_path == std::path::Path::new("accounts.dat") {
            c.accounts_path = data_dir.join("accounts.dat");
        }
        (p, c)
    };

    // Decode the embedded logo for the window icon.
    let icon = {
        let png = include_bytes!("../../assets/Logo.png");
        let img = image::load_from_memory(png).expect("failed to decode Logo.png");
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        eframe::egui::IconData {
            rgba: rgba.into_raw(),
            width: w,
            height: h,
        }
    };

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([config.window_width, config.window_height])
            .with_min_inner_size([640.0, 400.0])
            .with_title(format!(
                "Better Roblox Manager v{}",
                env!("CARGO_PKG_VERSION")
            ))
            .with_icon(icon),
        ..Default::default()
    };

    let _ = eframe::run_native(
        "Better Roblox Manager",
        native_options,
        Box::new(move |cc| {
            // Enable image loading for egui_extras (avatars, etc.)
            egui_extras::install_image_loaders(&cc.egui_ctx);
            install_cjk_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx, theme::by_id(&config.theme), config.panel_opacity);
            Ok(Box::new(app::AppState::new(config, config_path)))
        }),
    );
}

/// Register system CJK fonts as fallbacks.
///
/// egui's bundled fonts cover Latin, Greek and Cyrillic only, so Japanese,
/// Chinese and Korean text renders as tofu boxes. That shows up in artist
/// credits from the image APIs and in Roblox display names, both of which are
/// frequently CJK.
///
/// Rather than bundling a font (Noto CJK is tens of megabytes per weight) we
/// borrow what Windows already ships. Fonts are appended *after* the defaults
/// so they act purely as fallbacks: egui walks the family in order and uses
/// the first font containing each glyph, leaving Latin text untouched.
///
/// Each script takes the first candidate that exists, so we hold one font per
/// script rather than all of them. Missing fonts are skipped silently — a
/// system without Korean support simply keeps showing boxes for Hangul, which
/// is no worse than the current behaviour.
fn install_cjk_fonts(ctx: &eframe::egui::Context) {
    use eframe::egui::{FontData, FontDefinitions, FontFamily};

    // Per script: candidates in preference order. `.ttc` files are font
    // *collections*, hence the face index — `FontData::from_owned` defaults it
    // to 0, which is the right face for all of these.
    const GROUPS: &[(&str, &[(&str, u32)])] = &[
        // Japanese first: kana plus the Japanese variants of shared Han glyphs.
        ("cjk_jp", &[("meiryo.ttc", 0), ("YuGothR.ttc", 0), ("msgothic.ttc", 0)]),
        ("cjk_sc", &[("msyh.ttc", 0), ("simsun.ttc", 0)]),
        ("cjk_tc", &[("msjh.ttc", 0), ("mingliu.ttc", 0)]),
        ("cjk_kr", &[("malgun.ttf", 0), ("gulim.ttc", 0)]),
    ];

    let fonts_dir = std::path::PathBuf::from(
        std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string()),
    )
    .join("Fonts");

    let mut fonts = FontDefinitions::default();
    let mut loaded: Vec<String> = Vec::new();

    for (key, candidates) in GROUPS {
        for (file, index) in *candidates {
            let path = fonts_dir.join(file);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            // `.into()` rather than a bare `FontData`: this epaint version
            // stores font data as `Arc<FontData>` so it can be shared cheaply.
            fonts.font_data.insert(
                (*key).to_string(),
                FontData {
                    font: std::borrow::Cow::Owned(bytes),
                    index: *index,
                    tweak: Default::default(),
                }
                .into(),
            );
            loaded.push((*key).to_string());
            break; // one font per script is enough
        }
    }

    if loaded.is_empty() {
        tracing::warn!("No system CJK fonts found; non-Latin text will render as boxes");
        return;
    }

    for key in &loaded {
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push(key.clone());
        }
    }

    tracing::info!("Loaded CJK fallback fonts: {}", loaded.join(", "));
    ctx.set_fonts(fonts);
}
