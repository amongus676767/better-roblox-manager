//! Settings panel — global config, encryption toggles, multi-instance control.

use eframe::egui;
use ram_core::models::AppConfig;

/// Actions the settings panel can emit.
#[allow(dead_code)]
pub enum SettingsAction {
    SaveConfig,
    /// The user toggled the storage backend. The app layer must migrate the
    /// cookies *before* flipping `config.use_credential_manager`, so this
    /// carries the requested value rather than mutating the config here.
    SetStorageBackend { use_credential_manager: bool },
    /// Open a file picker for the background image.
    PickBackgroundImage,
    /// Forget the background image.
    ClearBackgroundImage,
    /// Open a file picker for a custom ambience sound.
    PickRainSound,
    /// Go back to the built-in synthesised rain.
    ClearRainSound,
    /// Download a fresh corner-overlay image.
    FetchOverlayImage,
    /// Remove the current corner-overlay image.
    ClearOverlayImage,
    ChangePassword { new_password: String },
    ClearPassword,
    EnableMultiInstance,
    DisableMultiInstance,
}

/// What the app knows about the currently loaded background image, so the
/// panel can report animation and failures without owning the texture cache.
#[derive(Clone, Copy)]
pub struct BackgroundInfo {
    pub broken: bool,
    pub frames: usize,
    pub truncated: bool,
}

/// Live status of the corner overlay, owned by the app.
#[derive(Clone)]
pub struct OverlayInfo {
    pub loading: bool,
    pub has_image: bool,
    pub credit: Option<String>,
    pub error: Option<String>,
}

/// Persistent state for the settings panel password change UI.
#[derive(Default)]
pub struct SettingsState {
    pub new_password_input: String,
    pub confirm_password_input: String,
}

/// Draw the settings UI. Returns `Some(SettingsAction)` when an action is triggered.
pub fn show(
    ui: &mut egui::Ui,
    config: &mut AppConfig,
    has_password: bool,
    settings_state: &mut SettingsState,
    roblox_running: bool,
    background: BackgroundInfo,
    overlay: OverlayInfo,
) -> Option<SettingsAction> {
    let mut action: Option<SettingsAction> = None;

    egui::ScrollArea::vertical().show(ui, |ui| {

    ui.heading("Settings");
    ui.separator();
    ui.add_space(8.0);

    let section_frame = egui::Frame::default()
        .inner_margin(egui::Margin::same(10.0))
        .rounding(egui::Rounding::same(6.0))
        .fill(ui.visuals().extreme_bg_color);

    // ---- Appearance ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Appearance");
        ui.add_space(4.0);

        let mut changed = false;

        // -- Background image --
        ui.horizontal(|ui| {
            if ui.button("\u{1f5bc} Choose background image...").clicked() {
                action = Some(SettingsAction::PickBackgroundImage);
            }
            if config.background_image.is_some() && ui.button("Clear").clicked() {
                action = Some(SettingsAction::ClearBackgroundImage);
            }
        });
        match &config.background_image {
            Some(path) => {
                let shown = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());
                if background.broken {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 80, 80),
                        format!("\u{26a0} Could not load {shown} \u{2014} moved, deleted, or an unsupported format."),
                    );
                } else {
                    let detail = if background.frames > 1 {
                        format!("{shown} \u{2014} animated, {} frames", background.frames)
                    } else {
                        shown
                    };
                    ui.label(
                        egui::RichText::new(detail)
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                    if background.truncated {
                        ui.colored_label(
                            egui::Color32::from_rgb(220, 160, 40),
                            "\u{26a0} Animation shortened to stay within the memory budget.",
                        );
                    }
                }
                changed |= ui
                    .add(
                        egui::Slider::new(&mut config.background_opacity, 0.0..=1.0)
                            .text("Image opacity"),
                    )
                    .changed();

                let current_fit = crate::background::Fit::from_id(&config.background_fit);
                egui::ComboBox::from_label("Image fit")
                    .selected_text(current_fit.label())
                    .show_ui(ui, |ui| {
                        for f in crate::background::Fit::ALL {
                            if ui.selectable_label(current_fit == f, f.label()).clicked() {
                                config.background_fit = f.id().to_string();
                                changed = true;
                            }
                        }
                    });
            }
            None => {
                ui.label(
                    egui::RichText::new("No image set \u{2014} using the theme colours below.")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            }
        }

        changed |= ui
            .add(egui::Slider::new(&mut config.background_dim, 0.0..=1.0).text("Background dim"))
            .on_hover_text(
                "Darkens the backdrop so panel text stays readable over a bright \
                 image, without washing the image out.",
            )
            .changed();

        changed |= ui
            .add(egui::Slider::new(&mut config.panel_opacity, 0.0..=1.0).text("Panel opacity"))
            .on_hover_text(
                "How solid the UI panels are. The panels cover the whole window, \
                 so this is the setting that actually lets a wallpaper show \
                 through. Low values trade readability for visibility.",
            )
            .changed();

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // -- Theme --
        let current = crate::theme::by_id(&config.theme);
        egui::ComboBox::from_label("Theme")
            .selected_text(current.name)
            .show_ui(ui, |ui| {
                for p in crate::theme::THEMES {
                    if ui.selectable_label(config.theme == p.id, p.name).clicked() {
                        config.theme = p.id.to_string();
                        changed = true;
                    }
                }
            });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // -- Animated effects --
        changed |= ui
            .checkbox(&mut config.effects_enabled, "Animated effects")
            .on_hover_text(
                "Off by default. While on, the app redraws continuously instead of \
                 idling between interactions, which costs a little CPU and battery.",
            )
            .changed();

        ui.add_enabled_ui(config.effects_enabled, |ui| {
            ui.indent("effect_toggles", |ui| {
                changed |= ui.checkbox(&mut config.effect_nebula, "Nebula clouds").changed();
                changed |= ui.checkbox(&mut config.effect_stars, "Starfield").changed();
                changed |= ui.checkbox(&mut config.effect_rain, "Rain").changed();
                changed |= ui
                    .checkbox(&mut config.effect_cursor_glow, "Cursor glow & trail")
                    .changed();
                ui.add_space(4.0);
                changed |= ui
                    .add(
                        egui::Slider::new(&mut config.effect_intensity, 0.0..=1.0)
                            .text("Effect intensity"),
                    )
                    .changed();
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // -- Ambience (independent of the animated effects above) --
        changed |= ui
            .checkbox(&mut config.rain_sound, "Rain sound")
            .on_hover_text(
                "Synthesised ambience \u{2014} no audio file needed. Works with or \
                 without the rain animation.",
            )
            .changed();
        ui.add_enabled_ui(config.rain_sound, |ui| {
            ui.indent("rain_volume", |ui| {
                changed |= ui
                    .add(egui::Slider::new(&mut config.rain_volume, 0.0..=1.0).text("Volume"))
                    .changed();
                ui.horizontal(|ui| {
                    if ui.button("\u{1f3b5} Use my own sound...").clicked() {
                        action = Some(SettingsAction::PickRainSound);
                    }
                    if config.rain_sound_file.is_some() && ui.button("Use built-in").clicked() {
                        action = Some(SettingsAction::ClearRainSound);
                    }
                });
                let label = match &config.rain_sound_file {
                    Some(p) => p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.to_string_lossy().to_string()),
                    None => "Built-in synthesised rain".to_string(),
                };
                ui.label(
                    egui::RichText::new(label)
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            });
        });

        if changed {
            action = Some(SettingsAction::SaveConfig);
        }
    });
    ui.add_space(6.0);

    // ---- Corner overlay ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Corner Image");
        ui.add_space(4.0);

        let mut changed = ui
            .checkbox(&mut config.overlay_enabled, "Show a corner image")
            .on_hover_text(
                "Pulls a random SFW anime image from nekos.best or nekos.life \
                 and draws it in a window corner.",
            )
            .changed();

        ui.add_enabled_ui(config.overlay_enabled, |ui| {
            ui.indent("overlay_opts", |ui| {
                ui.horizontal(|ui| {
                    if overlay.loading {
                        ui.spinner();
                        ui.label("Fetching...");
                    } else {
                        if ui.button("\u{1f504} New image").clicked() {
                            action = Some(SettingsAction::FetchOverlayImage);
                        }
                        if overlay.has_image && ui.button("Remove").clicked() {
                            action = Some(SettingsAction::ClearOverlayImage);
                        }
                    }
                });

                if let Some(err) = &overlay.error {
                    ui.colored_label(
                        egui::Color32::from_rgb(200, 80, 80),
                        format!("\u{26a0} {err}"),
                    );
                }
                if let Some(credit) = &overlay.credit {
                    ui.label(
                        egui::RichText::new(credit)
                            .small()
                            .color(ui.visuals().weak_text_color()),
                    );
                }

                changed |= ui
                    .add(egui::Slider::new(&mut config.overlay_opacity, 0.0..=1.0).text("Opacity"))
                    .changed();
                changed |= ui
                    .add(egui::Slider::new(&mut config.overlay_size, 0.05..=0.9).text("Size"))
                    .on_hover_text("Height as a fraction of the window.")
                    .changed();

                let current = crate::overlay::Corner::from_id(&config.overlay_corner);
                egui::ComboBox::from_label("Corner")
                    .selected_text(current.label())
                    .show_ui(ui, |ui| {
                        for c in crate::overlay::Corner::ALL {
                            if ui.selectable_label(current == c, c.label()).clicked() {
                                config.overlay_corner = c.id().to_string();
                                changed = true;
                            }
                        }
                    });

                changed |= ui
                    .checkbox(&mut config.overlay_show_credit, "Show artist credit")
                    .on_hover_text(
                        "nekos.best supplies the artist's name; crediting them is \
                         the least an app displaying their work can do.",
                    )
                    .changed();
                changed |= ui
                    .checkbox(&mut config.overlay_fetch_on_start, "New image on each launch")
                    .changed();
            });
        });

        if changed {
            action = Some(SettingsAction::SaveConfig);
        }
    });
    ui.add_space(6.0);

    // ---- Storage backend ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Storage");
        ui.add_space(4.0);
        // Bound to a local copy on purpose: flipping the config field directly
        // is what stranded users' cookies in the backend they just switched
        // away from. The app layer migrates first and only then commits.
        let mut wants_cm = config.use_credential_manager;
        if ui
            .checkbox(
                &mut wants_cm,
                "Use Windows Credential Manager (instead of encrypted file)",
            )
            .changed()
        {
            action = Some(SettingsAction::SetStorageBackend {
                use_credential_manager: wants_cm,
            });
        }
        if !config.use_credential_manager && !has_password {
            ui.colored_label(
                egui::Color32::from_rgb(220, 160, 40),
                "\u{26a0} Unlock the account store before switching backends, \
                 otherwise the saved cookies cannot be migrated.",
            );
        }
    });
    ui.add_space(6.0);

    // ---- Launch Behavior ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Launch Behavior");
        ui.add_space(4.0);

        let mut wants_multi = config.multi_instance_enabled;
        let toggled = ui.checkbox(
            &mut wants_multi,
            "Enable multi-instance",
        ).changed();
        if toggled {
            if wants_multi {
                action = Some(SettingsAction::EnableMultiInstance);
            } else {
                action = Some(SettingsAction::DisableMultiInstance);
            }
        }
        if config.multi_instance_enabled {
            ui.colored_label(
                egui::Color32::from_rgb(220, 160, 40),
                "\u{26a0} This interacts with Hyperion anti-cheat and may carry ban risk.",
            );
        }
        if !config.multi_instance_enabled && roblox_running {
            ui.colored_label(
                egui::Color32::from_rgb(180, 180, 180),
                "Close all Roblox processes (including tray) before enabling.",
            );
        }

        ui.add_space(4.0);
        ui.checkbox(
            &mut config.kill_background_roblox,
            "Kill Roblox tray/background processes automatically",
        ).on_hover_text("Kills idle \"always running\" Roblox processes (--launch-to-tray).");
        if config.multi_instance_enabled && !config.kill_background_roblox {
            ui.colored_label(
                egui::Color32::from_rgb(220, 160, 40),
                "⚠ Recommended when multi-instance is enabled. Tray processes stack up.",
            );
        }

        ui.add_space(4.0);
        ui.checkbox(
            &mut config.auto_arrange_windows,
            "Auto-arrange Roblox windows after launch",
        ).on_hover_text("Tiles Roblox windows in a grid (2 = side-by-side, 4 = 2×2, etc.).");

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("Launch delay:");
            let mut secs = config.launch_delay_secs as i32;
            ui.add(
                egui::DragValue::new(&mut secs)
                    .range(0..=300)
                    .speed(0.2)
                    .suffix(" s"),
            )
            .on_hover_text(
                "Minimum gap between account launches. Applies to single and bulk launches. 0 disables throttling.",
            );
            config.launch_delay_secs = secs.max(0) as u32;
            ui.label(
                egui::RichText::new("(Roblox rate-limits some IPs)")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
        });
    });
    ui.add_space(6.0);

    // ---- Privacy ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Privacy");
        ui.add_space(4.0);
        ui.checkbox(
            &mut config.privacy_mode,
            "Clear RobloxCookies.dat before each launch",
        ).on_hover_text("Prevents Roblox from associating your accounts via stored cookies.");
        ui.checkbox(
            &mut config.anonymize_names,
            "Anonymize account names",
        ).on_hover_text("Replaces usernames and display names with generic \"Account 1\", \"Account 2\", etc.");
    });
    ui.add_space(6.0);

    // ---- Roblox path override ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Roblox Player Path");
        ui.add_space(4.0);
        ui.label("Leave empty for auto-detect:");
        let mut path_str = config
            .roblox_player_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if ui.text_edit_singleline(&mut path_str).changed() {
            config.roblox_player_path = if path_str.trim().is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(path_str))
            };
        }
    });

    ui.add_space(12.0);

    if ui.button("💾  Save Settings").clicked() {
        action = Some(SettingsAction::SaveConfig);
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(8.0);

    // ---- Master password management ----
    section_frame.show(ui, |ui: &mut egui::Ui| {
        ui.set_min_width(ui.available_width());
        ui.strong("Master Password");
        ui.add_space(4.0);
        if has_password {
            ui.label("A master password is currently set.");
        } else {
            ui.colored_label(
                egui::Color32::from_rgb(220, 160, 40),
                "⚠ No master password set. Add an account to set one.",
            );
        }
        ui.add_space(4.0);

        ui.label("New password:");
        ui.add(
            egui::TextEdit::singleline(&mut settings_state.new_password_input)
                .password(true)
                .hint_text("Enter new password"),
        );
        ui.label("Confirm password:");
        ui.add(
            egui::TextEdit::singleline(&mut settings_state.confirm_password_input)
                .password(true)
                .hint_text("Confirm new password"),
        );
        ui.add_space(4.0);

        let passwords_match = !settings_state.new_password_input.is_empty()
            && settings_state.new_password_input == settings_state.confirm_password_input;

        if !settings_state.new_password_input.is_empty()
            && !settings_state.confirm_password_input.is_empty()
            && !passwords_match
        {
            ui.colored_label(
                egui::Color32::from_rgb(200, 60, 60),
                "Passwords do not match.",
            );
        }

        if ui
            .add_enabled(passwords_match, egui::Button::new("🔑  Change Password"))
            .clicked()
        {
            let new_pw = settings_state.new_password_input.clone();
            settings_state.new_password_input.clear();
            settings_state.confirm_password_input.clear();
            action = Some(SettingsAction::ChangePassword {
                new_password: new_pw,
            });
        }
    });

    }); // ScrollArea

    action
}
