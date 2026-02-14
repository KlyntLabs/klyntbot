//! Braille spinner for indicating ongoing operations.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use super::colors::{colorize, DIM};

/// Braille spinner patterns (8 frames)
const SPINNER_FRAMES: &[&str] = &["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

/// A braille spinner for indicating ongoing operations
pub struct Spinner {
    message: String,
    running: Arc<Mutex<bool>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    /// Creates a new spinner with the given message
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            running: Arc::new(Mutex::new(false)),
            thread_handle: None,
        }
    }

    /// Starts the spinner animation
    pub fn start(&mut self) {
        let running = Arc::clone(&self.running);
        *running.lock().unwrap() = true;

        let message = self.message.clone();
        let running_clone = Arc::clone(&running);

        let handle = thread::spawn(move || {
            let mut frame = 0;
            while *running_clone.lock().unwrap() {
                let spinner_char = SPINNER_FRAMES[frame % SPINNER_FRAMES.len()];
                print!("\r{} {}  ", colorize(spinner_char, DIM), message);
                io::stdout().flush().unwrap();

                frame += 1;
                thread::sleep(Duration::from_millis(100));
            }

            // Clear the spinner line completely
            print!("\r\x1b[K");
            io::stdout().flush().unwrap();
        });

        self.thread_handle = Some(handle);
    }

    /// Stops the spinner and clears the line
    pub fn stop(&mut self) {
        // Set the flag via Mutex::lock (not Arc::get_mut, which fails when
        // the spinner thread still holds a clone of the Arc).
        *self.running.lock().unwrap() = false;

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Updates the spinner message while it's running
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_creation() {
        let spinner = Spinner::new("testing");
        assert_eq!(spinner.message, "testing");
    }

    #[test]
    fn test_spinner_frames() {
        assert_eq!(SPINNER_FRAMES.len(), 8);
        assert_eq!(SPINNER_FRAMES[0], "⣾");
        assert_eq!(SPINNER_FRAMES[7], "⣷");
    }
}
