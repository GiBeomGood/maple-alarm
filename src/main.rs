#![windows_subsystem = "windows"]

use native_windows_gui as nwg;
use nwg::NativeUi;
use std::collections::HashMap;
use std::sync::Arc;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::Graphics::Gdi::{
    CreateSolidBrush, FillRect, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, SPI_GETWORKAREA, SWP_NOSIZE, SWP_NOZORDER, HWND_TOPMOST,
    SystemParametersInfoW, WM_CTLCOLORSTATIC, WM_ERASEBKGND,
};

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

    let reset_secs = resolve_reset_secs().unwrap_or(100);
    let shared_state = Arc::new(SharedState::new(reset_secs));

    nwg::init().expect("NWG init failed");
    nwg::Font::set_global_family("Malgun Gothic").expect("font failed");

    let app = ui::AlarmApp::build_ui(Default::default()).expect("UI build failed");
    app.set_shared_state(Arc::clone(&shared_state));

    // Hide controls that start invisible
    app.init_visibility();
    app.refresh_ui();

    // WM_CTLCOLORSTATIC handler for text colors on dark background
    let text_colors: HashMap<isize, u32> = {
        let mut m = HashMap::new();
        // light gray text on dark bg
        m.insert(hwnd_of(&app.label_caption_normal), 0x00_99_99_99u32);
        m.insert(hwnd_of(&app.label_time_normal),    0x00_e0_e0_e0u32);
        // red text (alarm state) – BGR: #ff4444 = 0x004444ff
        m.insert(hwnd_of(&app.label_caption_alarm),  0x00_44_44_ffu32);
        m.insert(hwnd_of(&app.label_time_alarm),     0x00_44_44_ffu32);
        // button text
        m.insert(hwnd_of(&app.btn_normal),            0x00_55_55_55u32); // dim
        m.insert(hwnd_of(&app.btn_alarm_light),       0x00_ff_ff_ffu32); // white
        m.insert(hwnd_of(&app.btn_alarm_dark),        0x00_cc_cc_ccu32); // light gray
        m.insert(hwnd_of(&app.btn_flash),             0x00_ff_ff_ffu32); // white
        m
    };

    // Pre-create background brushes per control
    // COLORREF is 0x00BBGGRR
    let bg_brushes: HashMap<isize, isize> = unsafe {
        let mut m = HashMap::new();
        let dark = CreateSolidBrush(0x00_1a_1a_1a) as isize;
        let btn_normal  = CreateSolidBrush(0x00_22_22_22) as isize;
        let btn_alarm   = CreateSolidBrush(0x00_44_44_ff) as isize; // #ff4444
        let btn_dark    = CreateSolidBrush(0x00_1e_1e_b4) as isize; // #b41e1e
        let btn_flash   = CreateSolidBrush(0x00_78_78_ff) as isize; // #ff7878

        for h in [
            hwnd_of(&app.label_caption_normal),
            hwnd_of(&app.label_caption_alarm),
            hwnd_of(&app.label_time_normal),
            hwnd_of(&app.label_time_alarm),
            hwnd_of(&app.dot_normal),
            hwnd_of(&app.dot_alarm),
        ] { m.insert(h, dark); }

        m.insert(hwnd_of(&app.btn_normal),      btn_normal);
        m.insert(hwnd_of(&app.btn_alarm_light), btn_alarm);
        m.insert(hwnd_of(&app.btn_alarm_dark),  btn_dark);
        m.insert(hwnd_of(&app.btn_flash),       btn_flash);
        m
    };

    let dark_brush = unsafe { CreateSolidBrush(0x00_1a_1a_1a) };

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
            None
        },
    ).expect("raw handler failed");

    // Position at bottom-right of work area
    let (x, y) = bottom_right_pos(180, 96, 12);
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
