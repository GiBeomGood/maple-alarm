use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

/// 공유 상태 구조체 — Atomic으로 데드락 방지
pub struct SharedState {
    pub remaining_secs: AtomicU64,  // 남은 시간(0 = 알람 중)
    pub alarm_active: AtomicBool,   // UI 상태 제어용 (테두리 색 등)
    pub blink_dark: AtomicBool,     // 깜빡임 어두운 상태 — WM_CTLCOLORSTATIC 핸들러와 공유
    pub reset_secs: u64,            // 리셋값 (기본 100, DEBUG_TIMER 시 다름)
    pub volume: AtomicU32,          // 볼륨 (0–200, 기본 100)
    pub vol_bar_open: AtomicBool,   // 볼륨 바 열림 상태
}

impl SharedState {
    pub fn new(reset_secs: u64) -> Arc<Self> {
        Arc::new(Self {
            remaining_secs: AtomicU64::new(reset_secs),
            alarm_active: AtomicBool::new(false),
            blink_dark: AtomicBool::new(false),
            reset_secs,
            volume: AtomicU32::new(100),
            vol_bar_open: AtomicBool::new(false),
        })
    }

    pub fn is_alarming(&self) -> bool {
        self.remaining_secs.load(Ordering::Acquire) == 0
    }
}
