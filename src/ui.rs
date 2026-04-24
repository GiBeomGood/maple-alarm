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

const BG: [u8; 3] = [26, 26, 26];
const BTN_NORMAL_BG: [u8; 3] = [40, 100, 200];
const BTN_ALARM_BG: [u8; 3] = [255, 68, 68];
const BTN_FLASH_BG: [u8; 3] = [255, 120, 120];
const DOT_GREEN: [u8; 3] = [68, 255, 136];
const DOT_RED: [u8; 3] = [255, 68, 68];

#[derive(NwgUi, Default)]
pub struct AlarmApp {
    #[nwg_control(
        size: (180, 128),
        title: "AlarmApp_v1_singleton_marker",
        flags: "POPUP | VISIBLE",
        ex_flags: WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_NOACTIVATE
    )]
    #[nwg_events(OnWindowClose: [AlarmApp::on_close])]
    pub window: nwg::Window,

    #[nwg_resource(source_bin: Some(include_bytes!("../assets/icon.ico")))]
    pub tray_icon_res: nwg::Icon,

    #[nwg_control(icon: Some(&data.tray_icon_res), tip: Some("Alarm"))]
    #[nwg_events(OnContextMenu: [AlarmApp::on_tray_menu])]
    pub tray: nwg::TrayNotification,

    #[nwg_control]
    #[nwg_events(OnNotice: [AlarmApp::on_tick])]
    pub tick_notice: nwg::Notice,

    // Fonts
    #[nwg_resource(family: "Consolas", size: 24, weight: 700)]
    pub font_time: nwg::Font,

    #[nwg_resource(family: "Malgun Gothic", size: 15)]
    pub font_small: nwg::Font,

    #[nwg_resource(family: "Malgun Gothic", size: 22, weight: 700)]
    pub font_btn: nwg::Font,

    // Status dot – normal (green), alarm (red)
    #[nwg_control(
        text: "",
        position: (12, 14),
        size: (8, 8),
        background_color: Some(DOT_GREEN)
    )]
    pub dot_normal: nwg::Label,

    #[nwg_control(
        text: "",
        position: (12, 14),
        size: (8, 8),
        background_color: Some(DOT_RED)
    )]
    pub dot_alarm: nwg::Label,

    // Caption labels
    #[nwg_control(
        text: "다음 알람까지",
        position: (2, 9),
        size: (176, 20),
        font: Some(&data.font_small),
        background_color: Some(BG),
        h_align: HTextAlign::Center
    )]
    #[nwg_events(OnMousePress: [AlarmApp::begin_drag])]
    pub label_caption_normal: nwg::Label,

    #[nwg_control(
        text: "알람",
        position: (2, 9),
        size: (176, 20),
        font: Some(&data.font_small),
        background_color: Some(BG),
        h_align: HTextAlign::Center
    )]
    #[nwg_events(OnMousePress: [AlarmApp::begin_drag])]
    pub label_caption_alarm: nwg::Label,

    // Time labels
    #[nwg_control(
        text: "--:--",
        position: (2, 33),
        size: (176, 34),
        font: Some(&data.font_time),
        background_color: Some(BG),
        h_align: HTextAlign::Center
    )]
    #[nwg_events(OnMousePress: [AlarmApp::begin_drag])]
    pub label_time_normal: nwg::Label,

    #[nwg_control(
        text: "0:00",
        position: (2, 33),
        size: (176, 34),
        font: Some(&data.font_time),
        background_color: Some(BG),
        h_align: HTextAlign::Center
    )]
    #[nwg_events(OnMousePress: [AlarmApp::begin_drag])]
    pub label_time_alarm: nwg::Label,

    // Normal state button
    #[nwg_control(
        text: "초기화",
        position: (8, 80),
        size: (164, 40),
        font: Some(&data.font_btn),
        background_color: Some(BTN_NORMAL_BG),
        h_align: HTextAlign::Center,
        v_align: VTextAlign::Center
    )]
    #[nwg_events(OnMousePress: [AlarmApp::on_reset])]
    pub btn_normal: nwg::Label,

    // Single alarm button — color driven by blink_dark via WM_PAINT handler
    #[nwg_control(
        text: "확인",
        position: (8, 80),
        size: (164, 40),
        font: Some(&data.font_btn),
        background_color: Some(BTN_ALARM_BG),
        h_align: HTextAlign::Center,
        v_align: VTextAlign::Center
    )]
    #[nwg_events(OnMousePress: [AlarmApp::on_confirm])]
    pub btn_alarm: nwg::Label,

    // Flash feedback button (brief highlight after confirm)
    #[nwg_control(
        text: "확인",
        position: (8, 80),
        size: (164, 40),
        font: Some(&data.font_btn),
        background_color: Some(BTN_FLASH_BG),
        h_align: HTextAlign::Center,
        v_align: VTextAlign::Center
    )]
    #[nwg_events(OnMousePress: [AlarmApp::on_confirm])]
    pub btn_flash: nwg::Label,

    // Blink timer (250 ms per phase)
    #[nwg_control(interval: std::time::Duration::from_millis(250))]
    #[nwg_events(OnTimerTick: [AlarmApp::on_blink])]
    pub blink_timer: nwg::AnimationTimer,

    // Flash feedback timer (140 ms)
    #[nwg_control(interval: std::time::Duration::from_millis(140))]
    #[nwg_events(OnTimerTick: [AlarmApp::on_confirm_flash_end])]
    pub flash_timer: nwg::AnimationTimer,

    pub shared_state: RefCell<Option<Arc<SharedState>>>,
    pub blink_running: RefCell<bool>,
    pub flash_active: RefCell<bool>,
}

impl AlarmApp {
    pub fn set_shared_state(&self, state: Arc<SharedState>) {
        *self.shared_state.borrow_mut() = Some(state);
    }

    pub fn init_visibility(&self) {
        self.dot_alarm.set_visible(false);
        self.label_caption_alarm.set_visible(false);
        self.label_time_alarm.set_visible(false);
        self.btn_alarm.set_visible(false);
        self.btn_flash.set_visible(false);
    }

    pub fn refresh_ui(&self) {
        let Some(ref state) = *self.shared_state.borrow() else { return };

        let remaining = state.remaining_secs.load(Ordering::Acquire);
        let is_alarm = remaining == 0;

        let time_text = format!("{}:{:02}", remaining / 60, remaining % 60);

        self.label_time_normal.set_text(&time_text);
        self.label_time_alarm.set_text(&time_text);

        if is_alarm {
            self.dot_normal.set_visible(false);
            self.dot_alarm.set_visible(true);
            self.label_caption_normal.set_visible(false);
            self.label_caption_alarm.set_visible(true);
            self.label_time_normal.set_visible(false);
            self.label_time_alarm.set_visible(true);

            if !*self.blink_running.borrow() {
                self.blink_timer.start();
                *self.blink_running.borrow_mut() = true;
                state.blink_dark.store(false, Ordering::Release);
                self.btn_normal.set_visible(false);
                self.btn_alarm.set_visible(true);
                self.btn_flash.set_visible(false);
            }
        } else {
            self.dot_normal.set_visible(true);
            self.dot_alarm.set_visible(false);
            self.label_caption_normal.set_visible(true);
            self.label_caption_alarm.set_visible(false);
            self.label_time_normal.set_visible(true);
            self.label_time_alarm.set_visible(false);

            if *self.blink_running.borrow() {
                self.blink_timer.stop();
                *self.blink_running.borrow_mut() = false;
            }
            state.blink_dark.store(false, Ordering::Release);

            // flash_active 중에는 btn_flash/btn_normal을 건드리지 않음
            if !*self.flash_active.borrow() {
                self.btn_normal.set_visible(true);
                self.btn_alarm.set_visible(false);
                self.btn_flash.set_visible(false);
            }
        }

        self.tray.set_tip(&format!("Alarm - {}", time_text));

        if let Some(hwnd) = self.window.handle.hwnd() {
            unsafe {
                use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
                InvalidateRect(hwnd as *mut _, std::ptr::null(), 1);
            }
        }
    }

    pub fn on_tick(&self) {
        self.refresh_ui();
    }

    pub fn on_confirm(&self) {
        let Some(ref state) = *self.shared_state.borrow() else { return };
        if !state.is_alarming() { return; }
        state.alarm_active.store(false, Ordering::Release);
        state.remaining_secs.store(state.reset_secs, Ordering::Release);

        self.blink_timer.stop();
        *self.blink_running.borrow_mut() = false;
        state.blink_dark.store(false, Ordering::Release);
        self.btn_alarm.set_visible(false);

        *self.flash_active.borrow_mut() = true;
        self.btn_flash.set_visible(true);
        self.flash_timer.start();

        alarm::play_confirm_sound();
        self.refresh_ui();
    }

    pub fn on_reset(&self) {
        let Some(ref state) = *self.shared_state.borrow() else { return };
        if state.is_alarming() { return; }
        if state.remaining_secs.load(Ordering::Acquire) == state.reset_secs { return; }
        state.remaining_secs.store(state.reset_secs, Ordering::Release);
        alarm::play_reset_sound();
        self.refresh_ui();
    }

    pub fn on_confirm_flash_end(&self) {
        self.flash_timer.stop();
        *self.flash_active.borrow_mut() = false;
        self.btn_flash.set_visible(false);
        self.btn_normal.set_visible(true);
    }

    pub fn on_blink(&self) {
        let Some(ref state) = *self.shared_state.borrow() else { return };
        if !state.is_alarming() { return; }
        let dark = state.blink_dark.load(Ordering::Acquire);
        state.blink_dark.store(!dark, Ordering::Release);

        if let Some(hwnd) = self.btn_alarm.handle.hwnd() {
            unsafe {
                use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
                InvalidateRect(hwnd as *mut _, std::ptr::null(), 1);
            }
        }
    }

    pub fn on_tray_menu(&self) {
        let Some(ref state) = *self.shared_state.borrow() else { return };

        let remaining = state.remaining_secs.load(Ordering::Acquire);
        let remaining_text = format!("남은 시간: {}:{:02}", remaining / 60, remaining % 60);

        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() { return; }

            let mut cursor = POINT { x: 0, y: 0 };
            GetCursorPos(&mut cursor);

            let hwnd = self.window.handle.hwnd().unwrap_or(std::ptr::null_mut());
            let remaining_w = to_wide(&remaining_text);

            AppendMenuW(menu, MF_STRING | MF_GRAYED, 1, remaining_w.as_ptr());
            AppendMenuW(menu, MF_STRING, 2, w!("종료"));

            SetForegroundWindow(hwnd as *mut _);
            let selected = TrackPopupMenu(
                menu,
                TPM_LEFTALIGN | TPM_RIGHTBUTTON | TPM_RETURNCMD,
                cursor.x, cursor.y,
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

    pub fn begin_drag(&self) {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
        unsafe {
            if let Some(hwnd) = self.window.handle.hwnd() {
                ReleaseCapture();
                SendMessageW(hwnd as *mut _, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
            }
        }
    }

    pub fn on_close(&self) {
        // Alt+F4 무시 – 종료는 트레이 메뉴에서만
    }
}

fn to_wide(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}
