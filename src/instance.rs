use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;
use windows_sys::core::w;

pub const WINDOW_TITLE: &str = "AlarmApp_v1_singleton_marker";
const SINGLETON_MUTEX_NAME: &str = "Global\\AlarmAppSingletonMutex_v1";

// CreateMutexW를 직접 FFI로 호출 (windows-sys에 없음)
extern "system" {
    fn CreateMutexW(
        lpMutexAttributes: *const std::ffi::c_void,
        bInitialOwner: BOOL,
        lpName: *const u16,
    ) -> HANDLE;
}

/// 단일 인스턴스 Mutex 획득 — 성공하면 true 반환
pub fn acquire_lock() -> bool {
    unsafe {
        let mutex = CreateMutexW(
            std::ptr::null(),
            TRUE,
            w!("Global\\AlarmAppSingletonMutex_v1"),
        );

        // HANDLE이 null이면 실패
        if mutex.is_null() {
            eprintln!("Failed to create singleton mutex: {}", SINGLETON_MUTEX_NAME);
            return true;
        }

        let err = GetLastError();
        if err == ERROR_ALREADY_EXISTS {
            // 이미 실행 중 — 기존 창 활성화 후 종료
            focus_existing_window();
            return false;
        }

        true
    }
}

/// 고유 제목으로 기존 창 찾아서 활성화
fn focus_existing_window() {
    unsafe {
        let hwnd = FindWindowW(std::ptr::null(), w!("AlarmApp_v1_singleton_marker"));
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_RESTORE);
            ShowWindow(hwnd, SW_SHOW);
            BringWindowToTop(hwnd);
            SetForegroundWindow(hwnd);
        } else {
            eprintln!("Existing window not found for singleton activation: {}", WINDOW_TITLE);
        }
    }
}
