# Native GPU waveforms

Waveforms are drawn by `crates/gfx` (wgpu, WGSL) into a surface that sits behind a transparent webview. The Svelte UI paints opaque panels everywhere except the rectangles where waveforms live; those are see-through holes. All pointer input stays in the webview.

## Layers

| Layer | Where | Platform knowledge |
| --- | --- | --- |
| Renderer, shaders, clock, layout, style, offscreen PNG harness | `crates/gfx` | none (wgpu only) |
| Render thread, snapshot slot, loader, IPC commands | `apps/desktop/src-tauri/src/native_view/{mod,render,shared,loader}.rs` | none |
| Host: creates the surface behind the webview | `native_view/{macos,windows,linux}.rs` behind `trait NativeHost` | one module per OS |
| Svelte wrapper, IPC wrapper, theme bridge, TS clock | `apps/desktop/src/lib/native/` | none |

IPC, style and state use CSS pixels plus a scale factor only.

## Status per platform

- macOS: implemented and verified (Metal, 60/75/120 Hz displays measured).
- Windows: `windows.rs` returns `Unsupported`; the UI falls back to the canvas renderer. Remaining: `transparent: true` window, WebView2 `DefaultBackgroundColor` alpha 0 (`ICoreWebView2Controller2::put_DefaultBackgroundColor`), a child `HWND` created below the WebView2 controller (`SetWindowPos(HWND_BOTTOM)`, `WS_CHILD | WS_CLIPSIBLINGS`, resized in `resize`), returned as `Win32WindowHandle`. If a child HWND cannot show through the transparent WebView2, switch to a DirectComposition visual behind the WebView2 visual. wgpu then uses DX12.
- Linux: `linux.rs` returns `Unsupported`. Remaining: transparent WebKitGTK webview (`WebView::set_background_color` alpha 0, ARGB visual) placed in a `gtk::Overlay` above a `gtk::DrawingArea` that owns the wgpu surface (Wayland or Xlib handles from the realized widget). wgpu uses Vulkan, falling back to GL.

Nothing else is platform specific; once a host returns a `RawTarget`, the render thread, clock and shaders are unchanged.

## Frame pacing

The render thread presents with `PresentMode::Fifo`. `get_current_texture` blocks until the display can take a frame, so frames follow the real refresh rate (60, 75, 120 and 144 Hz need no configuration). Position is extrapolated to `acquire time + measured frame interval - output latency`. Nothing is rendered when both decks are paused and no input changed: the thread waits on a condvar.

Time is sampled by the state pump every 3 ms while a deck moves (16 ms otherwise). `AudioSync` maps `framesProcessed` to the host clock with a min-filter, so poll jitter does not reach the playhead. `DeckClock` extrapolates with gentle error correction, never steps backwards unless the snapshot disagrees by more than 30 ms (a real seek or cue jump), and hands drag previews back to the engine without a visible jump.

## Look and scroll mapping

- Scrolling view: x is wall-clock, `(track secs - playhead) / tempoRatio * zoom`. Equal effective BPM gives equal beat spacing across decks.
- Columns are anchored to absolute track time at the current zoom and sampled from a max-reduction pyramid, so bars do not crawl. Sub-pixel scrolling blends neighbouring columns (`subPixel` style flag).
- Grid: downbeat is the first phrase start, else the grid's first beat. Beat ticks are faint, bar lines (4 beats) stronger and full height, 4-beat-bar lines (16 beats) deck-tinted and labelled 1, 5, 9. Lines are pixel snapped.
- Everything in the overview (phrase band, cues, loop, played dimming, playhead) and in the scrolling view (cue flags, loop, playhead) is drawn natively, so the HTML overlay canvases are not needed when native is active.

## IPC

`native_waveforms_available`, `native_set_rects`, `native_set_style`, `native_set_zoom`, `native_drag_preview`, `native_set_latency`, `native_reload_track`, `native_frame_stats`. `DECKFORGE_NATIVE_WAVEFORMS=0` forces the fallback. `DECKFORGE_NATIVE_LOG_STATS=1` logs frame statistics every 3 s while playing.

## Svelte integration

1. Replace the base waveform canvas in `ScrollWave.svelte` and `OverviewWave.svelte` with `<NativeSurface id="scroll-a" zoom={ZOOM_LEVELS[ui.zoomIndex]}>` (ids: `scroll-a`, `scroll-b`, `overview-a`, `overview-b`) and pass the existing canvas as the `fallback` snippet.
2. Keep the overlay canvas and `.hit` div as siblings above it, but only when `!native.available` for the overlay canvas. The `.hit` div must keep receiving pointer events.
3. Every ancestor between the holes and `<body>` must be unpainted (`html`, `body`, `.app`, `.decks`, `.scrolls`, the wave box). Panels around the holes stay opaque.
4. Drag: pass `nativeDragPreview` as the `onPreview` argument of `scrubGesture`/`absoluteGesture` when `native.available`, otherwise the existing `canvasPreview`. Seek commands still go out once per frame through the existing helper.
5. Call `initNative()` once at startup (the component also does). Theme variables are read from CSS custom properties listed in `styleBridge.ts`; unset ones keep the defaults.
6. Optional: `setNativeLatencyMs(ms)` to compensate output latency; `nativeFrameStats()` for a debug panel.
7. For the canvas fallback, `SmoothClock` in `native/clock.ts` is the same algorithm as the native clock.

## Manual checks

- Waveforms appear inside their boxes, nothing outside; resizing the window and dragging splitters keeps them in the boxes within a frame or two.
- Menus, dialogs and dropdowns cover the waveforms.
- Drag on a scrolling view (paused and playing): the waveform follows the pointer with no lag, no jump on release.
- Click and drag on an overview seeks.
- Both decks at the same effective BPM: bar lines line up vertically when in phase.
- Pause both decks: CPU drops to idle (no render thread wakeups).
- `DECKFORGE_NATIVE_WAVEFORMS=0` restores the canvas waveforms.
