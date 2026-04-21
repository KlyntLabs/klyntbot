use crate::types::WindowAction;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::Instant;

struct LastAction {
    action: WindowAction,
    timestamp: Instant,
    cycle_index: usize,
}

pub struct WindowManager {
    last_actions: Mutex<HashMap<u32, LastAction>>,
    pre_preset_cache: Mutex<HashMap<u32, (f64, f64, f64, f64)>>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            last_actions: Mutex::new(HashMap::new()),
            pre_preset_cache: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn execute(&self, action: &WindowAction) -> common::Result<()> {
        if let WindowAction::Preset(name) = action {
            return self.apply_preset(name);
        }

        use super::accessibility;

        let pid = accessibility::get_frontmost_pid().ok_or_else(|| {
            common::KlyntbotError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No frontmost window",
            ))
        })?;
        let screen = accessibility::get_screen_frame();
        let window_id = pid as u32;

        let cycle_index = {
            let mut last = self.last_actions.lock();
            let entry = last.get(&window_id);
            let idx = if let Some(prev) = entry {
                if std::mem::discriminant(&prev.action) == std::mem::discriminant(action)
                    && prev.timestamp.elapsed().as_secs() < 2
                {
                    (prev.cycle_index + 1) % 3
                } else {
                    0
                }
            } else {
                0
            };
            last.insert(
                window_id,
                LastAction {
                    action: action.clone(),
                    timestamp: Instant::now(),
                    cycle_index: idx,
                },
            );
            idx
        };

        let (x, y, w, h) = self.compute_frame(action, &screen, cycle_index);
        accessibility::set_window_frame(pid, x, y, w, h);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn execute(&self, _action: &WindowAction) -> common::Result<()> {
        Err(common::KlyntbotError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Window management only supported on macOS",
        )))
    }

    #[cfg(target_os = "macos")]
    pub fn apply_preset(&self, name: &str) -> common::Result<()> {
        use super::accessibility;
        use crate::window_mgmt::presets::lookup;

        let preset = lookup(name).ok_or_else(|| {
            common::KlyntbotError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("unknown preset: {name}"),
            ))
        })?;
        let pid = accessibility::get_frontmost_pid().ok_or_else(|| {
            common::KlyntbotError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no focused window",
            ))
        })?;
        let window_id = pid as u32;

        if name == "restore" {
            if let Some(prev) = self.pre_preset_cache.lock().get(&window_id).copied() {
                accessibility::set_window_frame(pid, prev.0, prev.1, prev.2, prev.3);
            }
            return Ok(());
        }

        let (sx, sy, sw, sh) = accessibility::get_screen_frame();
        // TODO(v2): capture current window rect for true restore; main-display frame only in v1.
        self.pre_preset_cache
            .lock()
            .insert(window_id, (sx, sy, sw, sh));
        let f = preset.frame;
        let x = sx + (f.x as f64) * sw;
        let y = sy + (f.y as f64) * sh;
        let w = (f.w as f64) * sw;
        let h = (f.h as f64) * sh;
        accessibility::set_window_frame(pid, x, y, w, h);
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    pub fn apply_preset(&self, _name: &str) -> common::Result<()> {
        Err(common::KlyntbotError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Window management only supported on macOS",
        )))
    }

    fn compute_frame(
        &self,
        action: &WindowAction,
        screen: &(f64, f64, f64, f64),
        cycle: usize,
    ) -> (f64, f64, f64, f64) {
        let (sx, sy, sw, sh) = *screen;
        let fractions = [0.5, 1.0 / 3.0, 2.0 / 3.0];
        let frac = fractions[cycle];

        match action {
            WindowAction::LeftHalf => (sx, sy, sw * frac, sh),
            WindowAction::RightHalf => (sx + sw * (1.0 - frac), sy, sw * frac, sh),
            WindowAction::TopHalf => (sx, sy, sw, sh * frac),
            WindowAction::BottomHalf => (sx, sy + sh * (1.0 - frac), sw, sh * frac),
            WindowAction::LeftThird => (sx, sy, sw / 3.0, sh),
            WindowAction::CenterThird => (sx + sw / 3.0, sy, sw / 3.0, sh),
            WindowAction::RightThird => (sx + sw * 2.0 / 3.0, sy, sw / 3.0, sh),
            WindowAction::Maximize => (sx, sy, sw, sh),
            WindowAction::Center | WindowAction::Restore => {
                let cw = sw * 0.6;
                let ch = sh * 0.7;
                (sx + (sw - cw) / 2.0, sy + (sh - ch) / 2.0, cw, ch)
            }
            // Preset variant is handled before compute_frame is called
            WindowAction::Preset(_) => (sx, sy, sw, sh),
        }
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn preset_frame_math_left_half_on_1920_1080() {
        let p = crate::window_mgmt::presets::lookup("left-half").unwrap();
        let w = (p.frame.w as f64 * 1920.0) as i32;
        let h = (p.frame.h as f64 * 1080.0) as i32;
        assert_eq!(w, 960);
        assert_eq!(h, 1080);
    }
}
