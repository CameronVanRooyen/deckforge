use std::ffi::c_void;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWindowOrderingMode};
use raw_window_handle::{AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle};
use tauri::WebviewWindow;

use super::{HostError, NativeHost, RawTarget};

#[derive(Default)]
pub struct MacHost {
    view: Option<Retained<NSView>>,
}

impl NativeHost for MacHost {
    fn attach(&mut self, window: &WebviewWindow) -> Result<RawTarget, HostError> {
        let mtm = MainThreadMarker::new().ok_or_else(|| HostError::Failed("attach must run on the main thread".into()))?;
        let content_ptr = window.ns_view().map_err(|e| HostError::Failed(e.to_string()))?;
        let content = unsafe { (content_ptr as *const NSView).as_ref() }.ok_or_else(|| HostError::Failed("window has no content view".into()))?;
        let view = NSView::initWithFrame(mtm.alloc::<NSView>(), content.bounds());
        view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);
        view.setWantsLayer(true);
        content.addSubview_positioned_relativeTo(&view, NSWindowOrderingMode::Below, None);
        let handle = NonNull::new(Retained::as_ptr(&view) as *mut c_void).expect("retained view is non-null");
        self.view = Some(view);
        Ok(RawTarget {
            window: RawWindowHandle::AppKit(AppKitWindowHandle::new(handle)),
            display: RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
        })
    }

    fn resize(&mut self, _width_px: u32, _height_px: u32) {}

    fn detach(&mut self) {
        if let Some(view) = self.view.take() {
            view.removeFromSuperview();
        }
    }
}
