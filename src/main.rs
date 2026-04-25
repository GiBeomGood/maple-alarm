#![windows_subsystem = "windows"]

use native_windows_gui as nwg;
use nwg::NativeUi;
use std::collections::HashMap;
use std::sync::Arc;
use windows_sys::Win32::Foundation::RECT;
use std::sync::atomic::Ordering;
use windows_sys::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DrawTextW, EndPaint, FillRect, FrameRect,
    PAINTSTRUCT, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
    DT_CENTER, DT_RIGHT, DT_VCENTER, DT_SINGLELINE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, DrawIconEx, DI_NORMAL,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PARENTNOTIFY,
    SetWindowPos, SPI_GETWORKAREA, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, HWND_TOPMOST,
    SystemParametersInfoW, WM_CTLCOLORSTATIC, WM_ERASEBKGND, WM_MOVING, WM_PAINT,
    GetWindowRect, IsWindowVisible, ShowWindow, SW_HIDE,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{SetCapture, ReleaseCapture};
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::Graphics::Gdi::InvalidateRect;

mod state;
mod timer;
mod ui;
mod alarm;
mod instance;

use state::SharedState;

fn main() {
    if !instance::acquire_lock() {
        return;
    }

    let reset_secs = resolve_reset_secs().unwrap_or(105);
    let shared_state = Arc::new(SharedState::new(reset_secs));

    nwg::init().expect("NWG init failed");
    nwg::Font::set_global_family("Malgun Gothic").expect("font failed");

    let app = ui::AlarmApp::build_ui(Default::default()).expect("UI build failed");
    app.set_shared_state(Arc::clone(&shared_state));

    // Hide controls that start invisible
    app.init_visibility();
    app.refresh_ui();

    // WM_CTLCOLORSTATIC handler for label text/background colors (buttons excluded — they own WM_PAINT)
    let text_colors: HashMap<isize, u32> = {
        let mut m = HashMap::new();
        m.insert(hwnd_of(&app.label_caption_normal), 0x00_99_99_99u32);
        m.insert(hwnd_of(&app.label_time_normal),    0x00_e0_e0_e0u32);
        m.insert(hwnd_of(&app.label_caption_alarm),  0x00_44_44_ffu32);
        m.insert(hwnd_of(&app.label_time_alarm),     0x00_44_44_ffu32);
        m
    };

    // Pre-create background brushes per control
    // COLORREF is 0x00BBGGRR
    let (bg_brushes, btn_normal_brush, btn_alarm_light_brush, btn_alarm_dark_brush) = unsafe {
        let mut m = HashMap::new();
        let dark          = CreateSolidBrush(0x00_1a_1a_1a) as isize;
        let btn_normal    = CreateSolidBrush(0x00_c8_64_28) as isize; // RGB(40,100,200)
        let btn_alarm_l   = CreateSolidBrush(0x00_44_44_ff) as isize; // #ff4444
        let btn_alarm_d   = CreateSolidBrush(0x00_1e_1e_b4) as isize; // #b41e1e

        for h in [
            hwnd_of(&app.label_caption_normal),
            hwnd_of(&app.label_caption_alarm),
            hwnd_of(&app.label_time_normal),
            hwnd_of(&app.label_time_alarm),
            hwnd_of(&app.dot_normal),
            hwnd_of(&app.dot_alarm),
        ] { m.insert(h, dark); }

        (m, btn_normal, btn_alarm_l, btn_alarm_d)
    };

    // Pre-create border brushes to avoid GDI alloc on every repaint
    let border_brush_normal = unsafe { CreateSolidBrush(0x00_44_44_44) };
    let border_brush_alarm  = unsafe { CreateSolidBrush(0x00_44_44_ff) };
    let dark_brush = unsafe { CreateSolidBrush(0x00_1a_1a_1a) };
    let border_state = Arc::clone(&shared_state);

    let vol_icon_hicon: *mut std::ffi::c_void = app.vol_icon_res.handle as *mut _;
    let window_hwnd_h = app.window.handle.hwnd().map(|h| h as isize).unwrap_or(0);
    let vol_window_hwnd_h = app.vol_window.handle.hwnd().map(|h| h as isize).unwrap_or(0);

    let _color_handler = nwg::bind_raw_event_handler(
        &app.window.handle,
        0xffff_0001,
        move |hwnd, msg, w, l| {
            if msg == WM_ERASEBKGND {
                unsafe {
                    let hdc = w as windows_sys::Win32::Graphics::Gdi::HDC;
                    let mut rc: RECT = std::mem::zeroed();
                    GetClientRect(hwnd as *mut std::ffi::c_void, &mut rc);
                    FillRect(hdc, &rc, dark_brush);
                    let is_alarming = border_state.alarm_active.load(Ordering::Acquire);
                    let bb = if is_alarming { border_brush_alarm } else { border_brush_normal };
                    FrameRect(hdc, &rc, bb);
                    let rc_inner = RECT { left: rc.left+1, top: rc.top+1, right: rc.right-1, bottom: rc.bottom-1 };
                    FrameRect(hdc, &rc_inner, bb);
                }
                return Some(1);
            }
            if msg == WM_CTLCOLORSTATIC {
                let hdc = w as windows_sys::Win32::Graphics::Gdi::HDC;
                unsafe {
                    SetBkMode(hdc, TRANSPARENT as i32);
                    if let Some(&color) = text_colors.get(&l) {
                        SetTextColor(hdc, color);
                    }
                    if let Some(&brush) = bg_brushes.get(&l) {
                        return Some(brush);
                    }
                }
            }
            if msg == WM_MOVING {
                let _ = vol_window_hwnd_h;
                if vol_window_hwnd_h != 0 {
                    unsafe {
                        let wr = &*(l as *const RECT);
                        let is_visible = IsWindowVisible(vol_window_hwnd_h as *mut _) != 0;
                        if is_visible {
                            SetWindowPos(vol_window_hwnd_h as *mut _, std::ptr::null_mut(),
                                wr.left, wr.top - 28, 0, 0,
                                SWP_NOZORDER | SWP_NOSIZE | SWP_NOACTIVATE);
                        }
                    }
                }
            }
            None
        },
    ).expect("raw handler failed");

    // Capture font handle for button WM_PAINT handlers
    let font_btn_hfont = app.font_btn.handle;
    let blink_state_normal = Arc::clone(&shared_state);
    let blink_state_alarm  = Arc::clone(&shared_state);

    let _btn_normal_handler = nwg::bind_raw_event_handler(
        &app.btn_normal.handle,
        0xffff_0002,
        move |hwnd, msg, w, _l| match msg {
            WM_ERASEBKGND => Some(1),
            WM_PAINT => {
                let _ = &blink_state_normal;
                unsafe {
                    let mut ps: PAINTSTRUCT = std::mem::zeroed();
                    let hdc = BeginPaint(hwnd as *mut _, &mut ps);
                    let mut rc: RECT = std::mem::zeroed();
                    GetClientRect(hwnd as *mut _, &mut rc);
                    FillRect(hdc, &rc, btn_normal_brush as *mut _);
                    SetBkMode(hdc, TRANSPARENT as i32);
                    SetTextColor(hdc, 0x00_ff_ff_ff);
                    let old = SelectObject(hdc, font_btn_hfont as *mut _);
                    let text: Vec<u16> = "초기화".encode_utf16().chain(Some(0)).collect();
                    DrawTextW(hdc, text.as_ptr(), -1, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                    SelectObject(hdc, old);
                    EndPaint(hwnd as *mut _, &ps);
                }
                Some(0)
            }
            _ => { let _ = w; None }
        },
    ).expect("btn_normal handler failed");

    let _btn_alarm_handler = nwg::bind_raw_event_handler(
        &app.btn_alarm.handle,
        0xffff_0003,
        move |hwnd, msg, w, _l| match msg {
            WM_ERASEBKGND => Some(1),
            WM_PAINT => {
                unsafe {
                    let mut ps: PAINTSTRUCT = std::mem::zeroed();
                    let hdc = BeginPaint(hwnd as *mut _, &mut ps);
                    let mut rc: RECT = std::mem::zeroed();
                    GetClientRect(hwnd as *mut _, &mut rc);
                    let dark = blink_state_alarm.blink_dark.load(Ordering::Acquire);
                    let brush = if dark { btn_alarm_dark_brush } else { btn_alarm_light_brush };
                    FillRect(hdc, &rc, brush as *mut _);
                    SetBkMode(hdc, TRANSPARENT as i32);
                    SetTextColor(hdc, 0x00_ff_ff_ff);
                    let old = SelectObject(hdc, font_btn_hfont as *mut _);
                    let text: Vec<u16> = "확인".encode_utf16().chain(Some(0)).collect();
                    DrawTextW(hdc, text.as_ptr(), -1, &mut rc, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
                    SelectObject(hdc, old);
                    EndPaint(hwnd as *mut _, &ps);
                }
                Some(0)
            }
            _ => { let _ = w; None }
        },
    ).expect("btn_alarm handler failed");

    let _vol_icon_handler = nwg::bind_raw_event_handler(
        &app.vol_icon_ctrl.handle,
        0xffff_0005,
        move |hwnd, msg, _w, _l| match msg {
            WM_ERASEBKGND => Some(1),
            WM_PAINT => {
                unsafe {
                    let mut ps: PAINTSTRUCT = std::mem::zeroed();
                    let hdc = BeginPaint(hwnd as *mut _, &mut ps);
                    let mut rc: RECT = std::mem::zeroed();
                    GetClientRect(hwnd as *mut _, &mut rc);
                    FillRect(hdc, &rc, dark_brush);
                    if !vol_icon_hicon.is_null() {
                        DrawIconEx(hdc, 0, 0, vol_icon_hicon, 16, 16, 0, std::ptr::null_mut(), DI_NORMAL);
                    }
                    EndPaint(hwnd as *mut _, &ps);
                }
                Some(0)
            }
            _ => None,
        },
    ).expect("vol_icon handler failed");

    // WM_PARENTNOTIFY on main window — catches clicks on children that overlap vol_btn area
    // (e.g. label_caption_normal overlaps x=152..178, y=9..28)
    let vol_toggle_state = Arc::clone(&shared_state);
    let _vol_toggle_handler = nwg::bind_raw_event_handler(
        &app.window.handle,
        0xffff_0007,
        move |_hwnd, msg, w, l| {
            let coords = if msg == WM_LBUTTONDOWN {
                let x = (l & 0xFFFF) as u16 as i16 as i32;
                let y = ((l >> 16) & 0xFFFF) as u16 as i16 as i32;
                Some((x, y))
            } else if msg == WM_PARENTNOTIFY && (w & 0xFFFF) as u32 == WM_LBUTTONDOWN as u32 {
                let x = (l & 0xFFFF) as u16 as i16 as i32;
                let y = ((l >> 16) & 0xFFFF) as u16 as i16 as i32;
                Some((x, y))
            } else {
                None
            };
            if let Some((x, y)) = coords {
                if x >= 152 && y >= 0 && y <= 28 {
                    let was_open = vol_toggle_state.vol_bar_open.load(Ordering::Acquire);
                    vol_toggle_state.vol_bar_open.store(!was_open, Ordering::Release);
                    let now_open = !was_open;
                    unsafe {
                        if now_open {
                            let mut wr: RECT = std::mem::zeroed();
                            GetWindowRect(window_hwnd_h as *mut _, &mut wr);
                            SetWindowPos(vol_window_hwnd_h as *mut _, std::ptr::null_mut(),
                                wr.left, wr.top - 28, 180, 28, SWP_NOZORDER | SWP_SHOWWINDOW);
                        } else {
                            ShowWindow(vol_window_hwnd_h as *mut _, SW_HIDE);
                        }
                        InvalidateRect(window_hwnd_h as *mut _, std::ptr::null(), 1);
                    }
                    return Some(0);
                }
            }
            None
        },
    ).expect("vol_toggle handler failed");

    let font_vol_hfont = app.font_vol.handle;
    let vol_bar_state = Arc::clone(&shared_state);
    let vol_dragging = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let track_brush = unsafe { CreateSolidBrush(0x00_44_44_44) };
    let fill_brush  = unsafe { CreateSolidBrush(0x00_99_99_99) };
    let thumb_brush = unsafe { CreateSolidBrush(0x00_ff_ff_ff) };
    let _vol_bar_handler = nwg::bind_raw_event_handler(
        &app.vol_window.handle,
        0xffff_0006,
        move |hwnd, msg, _w, l| {
            let set_vol_from_x = |x: i32| {
                let new_vol = ((x - 8) * 200 / 140).clamp(0, 200) as u32;
                vol_bar_state.volume.store(new_vol, Ordering::Relaxed);
                alarm::set_volume(new_vol);
                unsafe { InvalidateRect(hwnd as *mut _, std::ptr::null(), 0); }
            };
            match msg {
                WM_ERASEBKGND => Some(1),
                WM_PAINT => {
                    unsafe {
                        let mut ps: PAINTSTRUCT = std::mem::zeroed();
                        let hdc = BeginPaint(hwnd as *mut _, &mut ps);
                        let mut rc: RECT = std::mem::zeroed();
                        GetClientRect(hwnd as *mut _, &mut rc);
                        FillRect(hdc, &rc, dark_brush);
                        let vol = vol_bar_state.volume.load(Ordering::Relaxed);
                        let thumb_x = 8 + (vol as i32 * 140 / 200);
                        let track = RECT { left: 8, top: 13, right: 148, bottom: 15 };
                        FillRect(hdc, &track, track_brush);
                        if thumb_x > 8 {
                            let filled = RECT { left: 8, top: 13, right: thumb_x, bottom: 15 };
                            FillRect(hdc, &filled, fill_brush);
                        }
                        let thumb = RECT { left: thumb_x - 7, top: 6, right: thumb_x + 7, bottom: 22 };
                        FillRect(hdc, &thumb, thumb_brush);
                        let old_font = SelectObject(hdc, font_vol_hfont as *mut _);
                        SetBkMode(hdc, TRANSPARENT as i32);
                        SetTextColor(hdc, 0x00_cc_cc_cc);
                        let vol_text: Vec<u16> = format!("{:>3}", vol).encode_utf16().chain(Some(0)).collect();
                        let mut text_rc = RECT { left: 150, top: 0, right: 178, bottom: 28 };
                        DrawTextW(hdc, vol_text.as_ptr(), -1, &mut text_rc,
                            DT_RIGHT | DT_VCENTER | DT_SINGLELINE);
                        SelectObject(hdc, old_font);
                        EndPaint(hwnd as *mut _, &ps);
                    }
                    Some(0)
                }
                WM_LBUTTONDOWN => {
                    let x = (l & 0xFFFF) as u16 as i16 as i32;
                    set_vol_from_x(x);
                    vol_dragging.store(true, Ordering::Relaxed);
                    unsafe { SetCapture(hwnd as *mut _); }
                    Some(0)
                }
                WM_MOUSEMOVE => {
                    if vol_dragging.load(Ordering::Relaxed) {
                        let x = (l & 0xFFFF) as u16 as i16 as i32;
                        set_vol_from_x(x);
                    }
                    None
                }
                WM_LBUTTONUP => {
                    vol_dragging.store(false, Ordering::Relaxed);
                    unsafe { ReleaseCapture(); }
                    Some(0)
                }
                _ => None,
            }
        },
    ).expect("vol_bar handler failed");

    // Position at bottom-right of work area
    let (x, y) = bottom_right_pos(180, 128, 12);
    unsafe {
        if let Some(hwnd) = app.window.handle.hwnd() {
            SetWindowPos(
                hwnd as *mut _,
                HWND_TOPMOST as *mut _,
                x, y, 0, 0,
                SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }

    // Start countdown thread
    let sender = app.tick_notice.sender();
    timer::spawn_with_tick(Arc::clone(&shared_state), move || {
        sender.notice();
    });

    nwg::dispatch_thread_events();
}

fn hwnd_of(label: &nwg::Label) -> isize {
    label.handle.hwnd().map(|p| p as isize).unwrap_or(0)
}

fn bottom_right_pos(w: i32, h: i32, margin: i32) -> (i32, i32) {
    unsafe {
        let mut rect = std::mem::zeroed::<RECT>();
        if SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect as *mut _ as *mut _, 0) == 0 {
            return (100, 100);
        }
        let dpi = GetDpiForSystem() as f32;
        let scale = dpi / 96.0;
        let x = ((rect.right as f32 / scale) - w as f32 - margin as f32) as i32;
        let y = ((rect.bottom as f32 / scale) - h as f32 - margin as f32) as i32;
        (x, y)
    }
}

fn resolve_reset_secs() -> Option<u64> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" | "-s" => {
                if let Some(v) = args.next() {
                    if let Ok(n) = v.parse::<u64>() { return Some(n); }
                }
            }
            _ => {
                if let Ok(n) = arg.parse::<u64>() { return Some(n); }
            }
        }
    }
    std::env::var("DEBUG_TIMER").ok().and_then(|v| v.parse().ok())
}
