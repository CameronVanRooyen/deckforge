use tauri::WebviewWindow;

use super::{HostError, NativeHost, RawTarget};

/// Linux host is not implemented yet; `<NativeSurface>` falls back to the canvas renderer.
/// Plan: transparent WebKitGTK webview in a `gtk::Overlay` over a GTK drawing area that owns the wgpu surface.
#[derive(Default)]
pub struct LinuxHost;

impl NativeHost for LinuxHost {
    fn attach(&mut self, _window: &WebviewWindow) -> Result<RawTarget, HostError> {
        Err(HostError::Unsupported("Linux GTK overlay host is not implemented"))
    }

    fn resize(&mut self, _width_px: u32, _height_px: u32) {}

    fn detach(&mut self) {}
}
