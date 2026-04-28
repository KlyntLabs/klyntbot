//! `MacCapture` — ScreenCaptureKit-based screen capture on macOS.
//!
//! Uses the `screencapturekit` crate (a safe wrapper over `SCStream`).
//! Single-frame captures wrap `SCScreenshotManager.captureImage`.

use async_trait::async_trait;
use platform_capture::{
    AccessibilityNode, AxScope, CaptureError, DisplayInfo, Frame, PixelFormat, PlatformCapture,
    Result, WindowId, WindowInfo,
};
use platform_input::Rect;

pub struct MacCapture {
    /// Default display id used when `capture_screen(None)` is called
    /// without a region. Resolved at construction via `CGMainDisplayID`.
    default_display_id: u32,
}

impl MacCapture {
    pub fn new() -> Result<Self> {
        // SAFETY: CGMainDisplayID is documented thread-safe.
        extern "C" {
            fn CGMainDisplayID() -> u32;
        }
        let id = unsafe { CGMainDisplayID() };
        Ok(Self {
            default_display_id: id,
        })
    }
}

#[async_trait]
impl PlatformCapture for MacCapture {
    async fn capture_screen(&self, _region: Option<Rect>) -> Result<Frame> {
        use core_media_rs::cm_sample_buffer::CMSampleBuffer;
        use core_video_rs::cv_pixel_buffer::{lock::LockTrait, CVPixelBuffer};
        use screencapturekit::{
            shareable_content::SCShareableContent,
            stream::configuration::pixel_format::PixelFormat as SckPixelFormat,
            stream::configuration::SCStreamConfiguration, stream::content_filter::SCContentFilter,
            stream::screenshot_manager::capture,
        };
        use tokio::task;

        let display_id = self.default_display_id;
        let frame = task::spawn_blocking(move || -> Result<Frame> {
            let content = SCShareableContent::get()
                .map_err(|e| CaptureError::CaptureFailed(format!("SCShareableContent: {e:?}")))?;
            let display = content
                .displays()
                .into_iter()
                .find(|d| d.display_id() == display_id)
                .ok_or(CaptureError::DisplayNotFound(display_id))?;

            let filter = SCContentFilter::new().with_display_excluding_windows(&display, &[]);
            let cfg = SCStreamConfiguration::default()
                .set_width(display.width())
                .map_err(|e| CaptureError::CaptureFailed(format!("set_width: {e:?}")))?
                .set_height(display.height())
                .map_err(|e| CaptureError::CaptureFailed(format!("set_height: {e:?}")))?
                .set_pixel_format(SckPixelFormat::BGRA)
                .map_err(|e| CaptureError::CaptureFailed(format!("set_pixel_format: {e:?}")))?
                .set_scales_to_fit(false)
                .map_err(|e| CaptureError::CaptureFailed(format!("set_scales_to_fit: {e:?}")))?;

            let sample_buffer: CMSampleBuffer = capture(&filter, &cfg)
                .map_err(|e| CaptureError::CaptureFailed(format!("capture: {e:?}")))?;

            let pixel_buffer: CVPixelBuffer = sample_buffer
                .get_pixel_buffer()
                .map_err(|e| CaptureError::CaptureFailed(format!("get_pixel_buffer: {e:?}")))?;

            let width = pixel_buffer.get_width();
            let height = pixel_buffer.get_height();
            let bytes_per_row = pixel_buffer.get_bytes_per_row() as usize;
            let expected_len = height as usize * bytes_per_row;

            let data = {
                let lock = pixel_buffer
                    .lock()
                    .map_err(|e| CaptureError::CaptureFailed(format!("lock: {e:?}")))?;
                let slice = lock.as_slice();
                // slice lifetime is tied to lock; copy out.
                if slice.len() < expected_len {
                    return Err(CaptureError::CaptureFailed(format!(
                        "pixel buffer too small: {} < {}",
                        slice.len(),
                        expected_len
                    )));
                }
                slice[..expected_len].to_vec()
            };

            Ok(Frame {
                width,
                height,
                scale: 2.0, // TODO: replace with NSScreen.backingScaleFactor in Phase 2
                format: PixelFormat::Bgra8,
                data,
            })
        })
        .await
        .map_err(|e| CaptureError::CaptureFailed(format!("join error: {e}")))??;

        Ok(frame)
    }

    async fn capture_window(&self, _window_id: WindowId) -> Result<Frame> {
        Err(CaptureError::NotImplemented)
    }

    async fn list_displays(&self) -> Result<Vec<DisplayInfo>> {
        use platform_input::Rect;

        extern "C" {
            fn CGGetActiveDisplayList(
                max: u32,
                display_array: *mut u32,
                display_count: *mut u32,
            ) -> i32;
            fn CGDisplayBounds(display: u32) -> CGRect;
            fn CGDisplayBackingScaleFactor(display: u32) -> f64;
            fn CGMainDisplayID() -> u32;
        }
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct CGRect {
            origin: CGPoint,
            size: CGSize,
        }
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct CGPoint {
            x: f64,
            y: f64,
        }
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct CGSize {
            width: f64,
            height: f64,
        }

        const MAX_DISPLAYS: u32 = 32;
        let mut ids = [0u32; MAX_DISPLAYS as usize];
        let mut count: u32 = 0;
        // SAFETY: pointer + length passed correctly; CGGetActiveDisplayList
        // is documented thread-safe.
        let err = unsafe { CGGetActiveDisplayList(MAX_DISPLAYS, ids.as_mut_ptr(), &mut count) };
        if err != 0 {
            return Err(CaptureError::CaptureFailed(format!(
                "CGGetActiveDisplayList failed: {}",
                err
            )));
        }

        let main_id = unsafe { CGMainDisplayID() };
        let mut out = Vec::with_capacity(count as usize);
        for &id in ids.iter().take(count as usize) {
            let bounds = unsafe { CGDisplayBounds(id) };
            let scale = unsafe { CGDisplayBackingScaleFactor(id) };
            out.push(DisplayInfo {
                id,
                frame: Rect {
                    x: bounds.origin.x,
                    y: bounds.origin.y,
                    w: bounds.size.width,
                    h: bounds.size.height,
                },
                scale,
                name: format!("Display {}", id),
                is_primary: id == main_id,
            });
        }
        Ok(out)
    }

    async fn get_active_window(&self) -> Result<Option<WindowInfo>> {
        Err(CaptureError::NotImplemented)
    }

    async fn get_ax_tree(&self, scope: AxScope) -> Result<AccessibilityNode> {
        use crate::computer_use::ax_tree;
        use objc2::rc::Retained;
        use objc2_app_kit::NSWorkspace;
        use tokio::task;

        let pid = match scope {
            AxScope::ActiveApp => {
                // Resolve current frontmost app pid via NSWorkspace.
                // SAFETY: NSWorkspace is documented thread-safe in read-only contexts.
                let workspace: Retained<NSWorkspace> = NSWorkspace::sharedWorkspace();
                let app = workspace.frontmostApplication();
                let pid: i32 = app.map(|a| a.processIdentifier()).unwrap_or(0);
                if pid == 0 {
                    return Err(CaptureError::AxTreeUnavailable("no frontmost app".into()));
                }
                pid
            }
            AxScope::FullDesktop => {
                return Err(CaptureError::AxTreeUnavailable(
                    "FullDesktop scope not supported in Phase 1; use ActiveApp".into(),
                ))
            }
            AxScope::Window(_id) => {
                return Err(CaptureError::AxTreeUnavailable(
                    "Window scope deferred to Phase 4".into(),
                ))
            }
        };

        task::spawn_blocking(move || ax_tree::walk_focused_app(pid, 6))
            .await
            .map_err(|e| CaptureError::AxTreeUnavailable(format!("join: {e}")))?
    }
}
