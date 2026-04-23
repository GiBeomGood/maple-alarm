use native_windows_gui as nwg;
use native_windows_derive::NwgUi;
use crate::state::SharedState;
use crate::alarm;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::cell::RefCell;
use windows_sys::Win32::Foundation::POINT;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::w;

#[derive(NwgUi, Default)]
pub struct AlarmApp {
    #[nwg_control(
        size: (160, 72),
        title: "AlarmApp_v1_singleton_marker",
        flags: "POPUP | VISIBLE",
        ex_flags: WS_EX_TOOLWINDOW | WS_EX_TOPMOST
    )]
    #[nwg_events(OnMousePress: [AlarmApp::begin_drag], OnWindowClose: [AlarmApp::on_window_close])]
    pub window: nwg::Window,

    #[nwg_resource(source_bin: Some(include_bytes!("../assets/icon.ico")))]
    pub tray_icon: nwg::Icon,

    #[nwg_control]
    #[nwg_events(OnNotice: [AlarmApp::on_tick])]
    pub tick_notice: nwg::Notice,

    #[nwg_control(icon: Some(&data.tray_icon), tip: Some("Alarm"))]
    #[nwg_events(OnContextMenu: [AlarmApp::on_tray_menu])]
    pub tray: nwg::TrayNotification,

    #[nwg_control(text: "", position: (24, 12), size: (8, 8), background_color: Some([68, 255, 136]))]
    pub status_dot_normal: nwg::Label,

    #[nwg_control(text: "", position: (24, 12), size: (8, 8), background_color: Some([255, 68, 68]))]
    pub status_dot_alarm: nwg::Label,

    #[nwg_control(text: "다음 알람까지", position: (40, 9), size: (112, 14))]
    pub label_caption: nwg::Label,

    #[nwg_control(text: "1:40", position: (40, 28), size: (112, 20))]
    pub label_time: nwg::Label,

    #[nwg_control(text: "확인", position: (8, 48), size: (144, 20), background_color: Some([34, 34, 34]))]
    #[nwg_events(OnMousePress: [AlarmApp::on_confirm])]
    pub btn_label_light: nwg::Label,

    #[nwg_control(text: "확인", position: (8, 48), size: (144, 20), background_color: Some([255, 68, 68]))]
    #[nwg_events(OnMousePress: [AlarmApp::on_confirm])]
    pub btn_label_dark: nwg::Label,

    #[nwg_control(text: "확인", position: (8, 48), size: (144, 20), background_color: Some([255, 120, 120]))]
    #[nwg_events(OnMousePress: [AlarmApp::on_confirm])]
    pub btn_label_flash: nwg::Label,

    #[nwg_control(interval: std::time::Duration::from_millis(500))]
    #[nwg_events(OnTimerTick: [AlarmApp::on_blink])]
    pub blink_timer: nwg::AnimationTimer,

    #[nwg_control(interval: std::time::Duration::from_millis(140))]
    #[nwg_events(OnTimerTick: [AlarmApp::on_confirm_flash])]
    pub confirm_flash_timer: nwg::AnimationTimer,

    pub shared_state: RefCell<Option<Arc<SharedState>>>,
    pub blink_running: RefCell<bool>,
    pub blink_dark: RefCell<bool>,
    pub confirm_flash_running: RefCell<bool>,
}

impl AlarmApp {
    pub fn set_shared_state(&self, state: Arc<SharedState>) {
        *self.shared_state.borrow_mut() = Some(state);
    }

    pub fn refresh_ui(&self) {
        if let Some(ref state) = *self.shared_state.borrow() {
            let remaining = state.remaining_secs.load(Ordering::Acquire);
            let is_alarm = remaining == 0;
            let time_text = if is_alarm {
                "00:00".to_string()
            } else {
                format!("{}:{:02}", remaining / 60, remaining % 60)
            };

            self.status_dot_normal.set_visible(!is_alarm);
            self.status_dot_alarm.set_visible(is_alarm);
            self.label_time.set_text(&time_text);
            if is_alarm {
                self.label_caption.set_text("알람");
                self.btn_label_light.set_text("확인");
                self.btn_label_dark.set_text("확인");
                self.btn_label_light.set_visible(!*self.blink_dark.borrow());
                self.btn_label_dark.set_visible(*self.blink_dark.borrow());
                self.btn_label_flash.set_visible(false);
                if !*self.blink_running.borrow() {
                    self.blink_timer.start();
                    *self.blink_running.borrow_mut() = true;
                    *self.blink_dark.borrow_mut() = false;
                    self.btn_label_light.set_visible(true);
                    self.btn_label_dark.set_visible(false);
                }
            } else {
                self.label_caption.set_text("다음 알람까지");
                if *self.blink_running.borrow() {
                    self.blink_timer.stop();
                    *self.blink_running.borrow_mut() = false;
                }
                *self.blink_dark.borrow_mut() = false;
                self.btn_label_light.set_text("확인");
                self.btn_label_dark.set_text("확인");
                self.btn_label_light.set_visible(true);
                self.btn_label_dark.set_visible(false);
                self.btn_label_flash.set_visible(false);
            }

            let tooltip = format!("AlarmApp - {}", time_text);
            self.tray.set_tip(&tooltip);
        }
    }

    pub fn on_tick(&self) {
        self.refresh_ui();
    }

    pub fn on_confirm(&self) {
        if let Some(ref state) = *self.shared_state.borrow() {
            if state.is_alarming() {
                self.btn_label_flash.set_visible(true);
                self.btn_label_light.set_visible(false);
                self.btn_label_dark.set_visible(false);
                self.confirm_flash_timer.start();
                *self.confirm_flash_running.borrow_mut() = true;

                state.alarm_active.store(false, Ordering::Release);
                state.remaining_secs.store(state.reset_secs, Ordering::Release);
                self.blink_timer.stop();
                *self.blink_running.borrow_mut() = false;
                *self.blink_dark.borrow_mut() = false;
                self.btn_label_light.set_visible(true);
                self.btn_label_dark.set_visible(false);

                alarm::play_confirm_sound();
                self.refresh_ui();
            }
        }
    }

    pub fn on_confirm_flash(&self) {
        if *self.confirm_flash_running.borrow() {
            self.confirm_flash_timer.stop();
            *self.confirm_flash_running.borrow_mut() = false;
            self.btn_label_flash.set_visible(false);
        }
    }

    pub fn on_blink(&self) {
        if let Some(ref state) = *self.shared_state.borrow() {
            if state.is_alarming() {
                let dark = *self.blink_dark.borrow();
                if dark {
                    self.btn_label_light.set_visible(false);
                    self.btn_label_dark.set_visible(true);
                } else {
                    self.btn_label_light.set_visible(true);
                    self.btn_label_dark.set_visible(false);
                }
                *self.blink_dark.borrow_mut() = !dark;
            }
        }
    }

    pub fn on_tray_menu(&self) {
        if let Some(ref state) = *self.shared_state.borrow() {
            let remaining = state.remaining_secs.load(Ordering::Acquire);
            let mins = remaining / 60;
            let secs = remaining % 60;

            unsafe {
                let menu = CreatePopupMenu();
                if menu.is_null() {
                    return;
                }

                let mut cursor = POINT { x: 0, y: 0 };
                GetCursorPos(&mut cursor);

                let hwnd = self.window.handle.hwnd().unwrap_or(std::ptr::null_mut());
                let remaining_text = format!("남은 시간: {}:{:02}", mins, secs);
                let remaining_w = to_wide(&remaining_text);

                AppendMenuW(menu, MF_STRING | MF_GRAYED, 1, remaining_w.as_ptr());
                AppendMenuW(menu, MF_STRING, 2, w!("종료"));

                SetForegroundWindow(hwnd as *mut _);
                let selected = TrackPopupMenu(
                    menu,
                    TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
                    cursor.x,
                    cursor.y,
                    0,
                    hwnd as *mut _,
                    std::ptr::null(),
                );

                if selected == 2 {
                    nwg::stop_thread_dispatch();
                }

                DestroyMenu(menu);
            }
        }
    }

    pub fn begin_drag(&self) {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;

        unsafe {
            if let Some(hwnd) = self.window.handle.hwnd() {
                ReleaseCapture();
                SendMessageW(hwnd as *mut _, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0 as isize);
            }
        }
    }

    pub fn on_window_close(&self) {
        // Alt+F4를 무시함 - 종료는 오직 트레이 메뉴에서만 가능
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    let mut wide: Vec<u16> = value.encode_utf16().collect();
    wide.push(0);
    wide
}
