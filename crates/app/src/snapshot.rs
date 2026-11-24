//! `--snapshot FILE.png [--view VIEW]`: puts every imported file on the timeline, parks the
//! playhead, waits for the frames and filmstrips, saves a picture of the window and quits. For
//! seeing the UI while it runs off-screen. VIEW is a page (convert, compress, transcribe,
//! download, settings, each with the files on its list), focus, bare (no side panels), or menu
//! (the project menu open).

use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Result, anyhow};
use gpui::{Context, Window};

use crate::{Galdr, Page};

pub struct Snapshot {
    path: PathBuf,
    view: Option<String>,
    started: Instant,
    staged: Option<Instant>,
}

impl Snapshot {
    pub fn new(path: PathBuf, view: Option<String>) -> Self {
        Self { path, view, started: Instant::now(), staged: None }
    }
}

pub fn step(app: &mut Galdr, window: &mut Window, cx: &mut Context<Galdr>) {
    let Some(mut s) = app.snapshot.take() else { return };
    let loaded = !app.project.media.is_empty() && app.project.media.iter().all(|m| m.info.is_some() || m.error.is_some());
    match s.staged {
        None if loaded || s.started.elapsed().as_secs() > 10 => {
            for id in app.project.media.iter().filter(|m| m.info.is_some()).map(|m| m.id).collect::<Vec<_>>() {
                app.append(id);
            }
            if let Some(second) = app.project.clips.get(1).map(|c| c.id) {
                app.selected = Some(second);
            }
            let t = app.project.duration() * 0.3;
            app.preview.seek(&app.project, t, false);
            let paths: Vec<PathBuf> = app.project.media.iter().map(|m| m.path.clone()).collect();
            match s.view.as_deref() {
                Some("transcribe") => app.page = Page::Tool(0),
                Some("convert") => {
                    app.page = Page::Tool(crate::tools::CONVERT);
                    app.add_tool_files(crate::tools::CONVERT, paths, cx);
                }
                Some("compress") => {
                    app.page = Page::Tool(crate::tools::COMPRESS);
                    app.add_tool_files(crate::tools::COMPRESS, paths, cx);
                }
                Some("download") => app.page = Page::Tool(3),
                Some("settings") => app.page = Page::Settings,
                Some("focus") => app.focus_view = true,
                Some("menu") => app.menu = true,
                Some("bare") => app.set_layout(|l| {
                    l.media = false;
                    l.inspector = false;
                }),
                _ => {}
            }
            s.staged = Some(Instant::now());
        }
        Some(at) => {
            let strips = app.project.clips.iter().all(|c| app.strips.contains_key(&c.media));
            if (strips && at.elapsed().as_secs_f64() > 1.5) || at.elapsed().as_secs() > 10 {
                match capture(window, &s.path) {
                    Ok(()) => println!("saved {}", s.path.display()),
                    Err(e) => eprintln!("snapshot: {e:#}"),
                }
                cx.quit();
                return;
            }
        }
        None => {}
    }
    app.snapshot = Some(s);
}

#[cfg(windows)]
fn capture(window: &Window, path: &std::path::Path) -> Result<()> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
    };
    use windows::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let handle = HasWindowHandle::window_handle(window).map_err(|e| anyhow!("no window handle: {e}"))?;
    let RawWindowHandle::Win32(h) = handle.as_raw() else { return Err(anyhow!("not a Win32 window")) };
    let hwnd = HWND(h.hwnd.get() as *mut _);
    unsafe {
        let mut rect = RECT::default();
        GetClientRect(hwnd, &mut rect)?;
        let (w, h) = (rect.right - rect.left, rect.bottom - rect.top);
        let screen = GetDC(Some(hwnd));
        let dc = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, w, h);
        let old = SelectObject(dc, bitmap.into());
        // Client area only, and what DirectComposition draws (PW_RENDERFULLCONTENT).
        let ok = PrintWindow(hwnd, dc, PRINT_WINDOW_FLAGS(1 | 2)).as_bool();
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0u8; (w * h * 4) as usize];
        let lines = GetDIBits(dc, bitmap, 0, h as u32, Some(pixels.as_mut_ptr().cast()), &mut info, DIB_RGB_COLORS);
        SelectObject(dc, old);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(dc);
        ReleaseDC(Some(hwnd), screen);
        if !ok || lines == 0 {
            return Err(anyhow!("couldn't read the window's pixels"));
        }
        for p in pixels.chunks_exact_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        image::RgbaImage::from_raw(w as u32, h as u32, pixels)
            .ok_or_else(|| anyhow!("bad image size"))?
            .save(path)?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn capture(_window: &Window, _path: &std::path::Path) -> Result<()> {
    Err(anyhow!("--snapshot is only on Windows so far"))
}
