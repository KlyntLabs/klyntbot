//! Lazy window creation — auxiliary windows are created on first show
//! instead of at startup, saving ~1 GB+ of GPU memory from idle WebView
//! backing buffers.

use tauri::window::{Effect, EffectState, EffectsBuilder};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tracing::{info, warn};

use crate::commands::window::{WINDOW_LAUNCHER, WINDOW_QUICK_CAPTURE, WINDOW_TRAY};

/// Get an existing window or lazily create it on first access.
///
/// Returns `None` only if the label is unknown or the builder fails.
/// The window is created hidden — the caller is responsible for showing it.
pub fn get_or_create_window(app: &AppHandle, label: &str) -> Option<WebviewWindow> {
    if let Some(w) = app.get_webview_window(label) {
        return Some(w);
    }

    let window = match label {
        WINDOW_LAUNCHER => build_launcher(app),
        WINDOW_TRAY => build_tray(app),
        "distraction-overlay" => build_distraction_overlay(app),
        WINDOW_QUICK_CAPTURE => build_quick_capture(app),
        "voice-orb" => build_voice_orb(app),
        _ => {
            warn!("get_or_create_window: unknown label '{label}'");
            return None;
        }
    };

    match window {
        Ok(w) => {
            info!("Lazily created window '{label}'");
            Some(w)
        }
        Err(e) => {
            warn!("Failed to create window '{label}': {e}");
            None
        }
    }
}

// ── Shared helpers ───────────────────────────────────────────────────

fn hud_effects() -> tauri::utils::config::WindowEffectsConfig {
    EffectsBuilder::new()
        .effect(Effect::HudWindow)
        .state(EffectState::Active)
        .radius(16.0)
        .build()
}

fn dismiss_on_blur(window: &WebviewWindow) {
    let w = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            let _ = w.hide();
        }
    });
}

// ── Per-window builders ──────────────────────────────────────────────

fn build_launcher(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, WINDOW_LAUNCHER, WebviewUrl::App("/#/launcher".into()))
        .title("")
        .inner_size(660.0, 580.0)
        .resizable(false)
        .decorations(false)
        .visible(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .center()
        .effects(hud_effects())
        .build()?;

    // Enhanced dismiss-on-blur: hide + reset voice recording state
    let w = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Focused(false) = event {
            let _ = w.hide();
            let _ = w.emit("voice-recording-reset", ());
        }
    });

    Ok(window)
}

fn build_tray(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(app, WINDOW_TRAY, WebviewUrl::App("/#/tray".into()))
        .title("")
        .inner_size(320.0, 600.0)
        .resizable(false)
        .decorations(false)
        .visible(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .effects(hud_effects())
        .build()?;

    dismiss_on_blur(&window);
    Ok(window)
}

fn build_distraction_overlay(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(
        app,
        "distraction-overlay",
        WebviewUrl::App("/#/distraction-overlay".into()),
    )
    .title("")
    .inner_size(340.0, 300.0)
    .resizable(false)
    .decorations(false)
    .visible(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .center()
    .focused(true)
    .effects(hud_effects())
    .build()
}

fn build_quick_capture(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let window = WebviewWindowBuilder::new(
        app,
        WINDOW_QUICK_CAPTURE,
        WebviewUrl::App("/#/quick-capture".into()),
    )
    .title("")
    .inner_size(500.0, 200.0)
    .resizable(false)
    .decorations(false)
    .visible(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .center()
    .effects(hud_effects())
    .build()?;

    dismiss_on_blur(&window);
    Ok(window)
}

fn build_voice_orb(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, "voice-orb", WebviewUrl::App("/#/voice-orb".into()))
        .title("")
        .inner_size(200.0, 200.0)
        .resizable(false)
        .decorations(false)
        .visible(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(false)
        .build()
}
