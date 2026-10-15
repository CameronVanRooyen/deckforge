use tauri::WebviewWindow;

use super::{HostError, NativeHost, RawTarget};

/// Windows host is not implemented yet; `<NativeSurface>` falls back to the canvas renderer.
/// Plan: transparent Tauri window, WebView2 `DefaultBackgroundColor` alpha 0, child HWND behind the WebView2 controller.
#[derive(Default)]
pub struct WindowsHost;

impl NativeHost for WindowsHost {
    fn attach(&mut self, _window: &WebviewWindow) -> Result<RawTarget, HostError> {
        Err(HostError::Unsupported("Windows child-HWND host is not implemented"))
    }

    fn resize(&mut self, _width_px: u32, _height_px: u32) {}

    fn detach(&mut self) {}
}
