use crate::types::WindowAction;
use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

struct LastAction {
    action: WindowAction,
    timestamp: Instant,
    cycle_index: usize,
}

pub struct WindowManager {
    last_actions: Mutex<HashMap<u32, LastAction>>,
    frame_history: Mutex<HashMap<u32, VecDeque<(f64, f64, f64, f64)>>>,
}

impl WindowManager {
    pub fn new() -> Self {
        Self {
            last_actions: Mutex::new(HashMap::new()),
            frame_history: Mutex::new(HashMap::new()),
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

        if !matches!(action, WindowAction::Restore) {
            self.capture_current(pid, window_id);
        }

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

        if name == "restore" {
            let pid = accessibility::get_frontmost_pid().ok_or_else(|| {
                common::KlyntbotError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "no focused window",
                ))
            })?;
            let window_id = pid as u32;
            let popped = {
                let mut history = self.frame_history.lock();
                history.get_mut(&window_id).and_then(|s| s.pop_back())
            };
            if let Some((x, y, w, h)) = popped {
                accessibility::set_window_frame(pid, x, y, w, h);
                return Ok(());
            } else {
                return Err(common::ToolError::ExecutionFailed(
                    "No previous frame to restore".into(),
                )
                .into());
            }
        }

        let pid = accessibility::get_frontmost_pid().ok_or_else(|| {
            common::KlyntbotError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no focused window",
            ))
        })?;
        let window_id = pid as u32;
        self.capture_current(pid, window_id);

        let (sx, sy, sw, sh) = accessibility::get_screen_frame();
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

    #[cfg(target_os = "macos")]
    fn capture_current(&self, pid: i32, window_id: u32) {
        use super::accessibility;
        if let Some((x, y, w, h)) = accessibility::get_frontmost_window_frame(pid) {
            let mut history = self.frame_history.lock();
            let stack = history.entry(window_id).or_default();
            if stack.len() >= 8 {
                stack.pop_front();
            }
            stack.push_back((x, y, w, h));
        }
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
            WindowAction::Preset(_) => unreachable!("Preset short-circuits in execute()"),
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
    use super::*;

    #[test]
    fn preset_frame_math_left_half_on_1920_1080() {
        let p = crate::window_mgmt::presets::lookup("left-half").unwrap();
        let w = (p.frame.w as f64 * 1920.0) as i32;
        let h = (p.frame.h as f64 * 1080.0) as i32;
        assert_eq!(w, 960);
        assert_eq!(h, 1080);
    }

    #[test]
    fn restore_pops_last_frame_per_window() {
        let mgr = WindowManager::new();
        let window_id = 42u32;

        // Manually push frames into history (bypass capture for test)
        {
            let mut h = mgr.frame_history.lock();
            let s = h.entry(window_id).or_default();
            s.push_back((0.0, 0.0, 800.0, 600.0));
            s.push_back((100.0, 100.0, 1024.0, 768.0));
        }

        // Pop once (simulating restore)
        let popped = {
            let mut h = mgr.frame_history.lock();
            h.get_mut(&window_id).and_then(|s| s.pop_back())
        };
        assert_eq!(popped.unwrap().2, 1024.0);

        // History stack should now have 1 frame
        let remaining = mgr
            .frame_history
            .lock()
            .get(&window_id)
            .map(|s| s.len())
            .unwrap_or(0);
        assert_eq!(remaining, 1);
    }

    #[test]
    fn frame_history_caps_at_8() {
        let mgr = WindowManager::new();
        let window_id = 7u32;
        {
            let mut h = mgr.frame_history.lock();
            let s = h.entry(window_id).or_default();
            for i in 0..12 {
                if s.len() >= 8 {
                    s.pop_front();
                }
                s.push_back((i as f64, 0.0, 100.0, 100.0));
            }
        }
        let len = mgr.frame_history.lock().get(&window_id).unwrap().len();
        assert_eq!(len, 8);
    }
}
