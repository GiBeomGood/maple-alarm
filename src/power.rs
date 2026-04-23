use windows_sys::Win32::UI::WindowsAndMessaging::*;
use native_windows_gui as nwg;

const PBT_APMSUSPEND: u32 = 0x0004;

/// 전원 이벤트 핸들러 등록 (suspend 감지)
pub fn register_handlers(handle: &nwg::ControlHandle) -> Result<nwg::RawEventHandler, nwg::NwgError> {
    nwg::bind_raw_event_handler(handle, 0x10001, move |_hwnd, msg, wparam, _lparam| {
        if msg == WM_POWERBROADCAST && wparam as u32 == PBT_APMSUSPEND {
            nwg::stop_thread_dispatch();
            return Some(0);
        }

        None
    })
}
