#![windows_subsystem = "windows"]

use eframe::egui;
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::HiDpi::GetDpiForSystem;
use windows_sys::Win32::UI::WindowsAndMessaging::{SPI_GETWORKAREA, SystemParametersInfoW};

mod state;
mod timer;
mod app;
mod alarm;
mod instance;

use state::SharedState;

fn main() {
    if !instance::acquire_lock() {
        std::process::exit(0);
    }

    let reset_secs = resolve_reset_secs().unwrap_or(100);
    let state = SharedState::new(reset_secs);

    timer::spawn(state.clone());

    let (pos_x, pos_y) = bottom_right_pos(180, 96, 12);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([180.0, 96.0])
            .with_position([pos_x as f32, pos_y as f32])
            .with_always_on_top()
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false),
        ..Default::default()
    };

    eframe::run_native(
        "AlarmApp_v1_singleton_marker",
        native_options,
        Box::new(move |cc| {
            let (tray, exit_id) = build_tray();
            Ok(Box::new(app::AlarmApp::new(cc, state, tray, exit_id)))
        }),
    )
    .expect("eframe failed");
}

fn build_tray() -> (tray_icon::TrayIcon, tray_icon::menu::MenuId) {
    use tray_icon::menu::{Menu, MenuItem};
    use tray_icon::TrayIconBuilder;

    let exit_item = MenuItem::new("종료", true, None);
    let exit_id = exit_item.id().clone();

    let menu = Menu::new();
    menu.append(&exit_item).expect("menu append failed");

    let icon = load_tray_icon();

    let tray = TrayIconBuilder::new()
        .with_tooltip("AlarmApp")
        .with_menu(Box::new(menu))
        .with_icon(icon)
        .build()
        .expect("tray build failed");

    (tray, exit_id)
}

fn load_tray_icon() -> tray_icon::Icon {
    tray_icon::Icon::from_path("assets/icon.ico", Some((32, 32))).unwrap_or_else(|_| {
        let rgba = vec![255u8, 100, 100, 255].repeat(32 * 32);
        tray_icon::Icon::from_rgba(rgba, 32, 32).unwrap()
    })
}

fn bottom_right_pos(w: i32, h: i32, margin: i32) -> (i32, i32) {
    unsafe {
        let mut rect = std::mem::zeroed::<RECT>();
        if SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut rect as *mut _ as *mut _, 0) == 0 {
            return (100, 100);
        }

        let dpi = GetDpiForSystem() as f32;
        let scale = dpi / 96.0;

        let work_right = rect.right as f32 / scale;
        let work_bottom = rect.bottom as f32 / scale;

        let x = (work_right - w as f32 - margin as f32) as i32;
        let y = (work_bottom - h as f32 - margin as f32) as i32;

        (x, y)
    }
}

fn resolve_reset_secs() -> Option<u64> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" | "-s" => {
                if let Some(value) = args.next() {
                    if let Ok(parsed) = value.parse::<u64>() {
                        return Some(parsed);
                    }
                }
            }
            _ => {
                if let Ok(parsed) = arg.parse::<u64>() {
                    return Some(parsed);
                }
            }
        }
    }
    std::env::var("DEBUG_TIMER").ok().and_then(|v| v.parse().ok())
}
