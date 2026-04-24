use crate::state::SharedState;
use crate::alarm;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

pub fn spawn_with_tick<F: Fn() + Send + 'static>(state: Arc<SharedState>, on_tick: F) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));

            let cur = state.remaining_secs.load(Ordering::Acquire);
            if cur > 0 {
                let next = cur - 1;
                let ok = state.remaining_secs.compare_exchange(
                    cur, next,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                ).is_ok();

                if ok && next == 0 {
                    state.alarm_active.store(true, Ordering::Release);
                    alarm::start_beep_loop();
                }
            }

            on_tick();
        }
    });
}
