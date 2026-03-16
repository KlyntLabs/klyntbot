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
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            last_actions: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn execute(&self, action: &WindowAction) -> common::Result<()> {
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
        }
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}
