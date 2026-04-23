use crate::state::SharedState;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// Beep을 직접 kernel32에서 로드
extern "system" {
    fn Beep(freq: u32, duration: u32) -> i32;
}

/// 알람 비프 루프 스레드 시작
pub fn start_beep_loop(state: Arc<SharedState>) {
    thread::spawn(move || {
        while state.alarm_active.load(std::sync::atomic::Ordering::Acquire) {
            unsafe {
                Beep(1200, 80); // 1200Hz, 80ms
            }
            thread::sleep(Duration::from_millis(60));
        }
    });
}

/// 확인음 재생 (더블비프 1800→2400Hz)
pub fn play_confirm_sound() {
    thread::spawn(|| {
        unsafe {
            Beep(1800, 60);
            Beep(2400, 60);
        }
    });
}
