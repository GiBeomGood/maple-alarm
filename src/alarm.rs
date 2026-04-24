use std::sync::OnceLock;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender, RecvTimeoutError};
use std::thread;
use std::time::Duration;

extern "system" {
    fn Beep(freq: u32, duration: u32) -> i32;
}

enum SoundCmd {
    StartAlarm,
    Confirm,
    Reset,
}

static SOUND_TX: OnceLock<Mutex<Sender<SoundCmd>>> = OnceLock::new();

fn get_sender() -> &'static Mutex<Sender<SoundCmd>> {
    SOUND_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<SoundCmd>();
        thread::spawn(move || {
            let mut alarming = false;
            loop {
                if alarming {
                    unsafe { Beep(1200, 100); }
                    match rx.recv_timeout(Duration::from_millis(400)) {
                        Ok(SoundCmd::Confirm) => {
                            alarming = false;
                            unsafe { Beep(1800, 80); Beep(2400, 80); }
                            while matches!(rx.try_recv(), Ok(SoundCmd::Reset | SoundCmd::Confirm)) {}
                        }
                        Ok(SoundCmd::Reset) => {
                            alarming = false;
                            unsafe { Beep(2400, 80); Beep(1800, 80); }
                            while matches!(rx.try_recv(), Ok(SoundCmd::Reset | SoundCmd::Confirm)) {}
                        }
                        Ok(SoundCmd::StartAlarm) | Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match rx.recv() {
                        Ok(SoundCmd::StartAlarm) => { alarming = true; }
                        Ok(SoundCmd::Confirm) => {
                            unsafe { Beep(1800, 80); Beep(2400, 80); }
                            while matches!(rx.try_recv(), Ok(SoundCmd::Reset | SoundCmd::Confirm)) {}
                        }
                        Ok(SoundCmd::Reset) => {
                            unsafe { Beep(2400, 80); Beep(1800, 80); }
                            while matches!(rx.try_recv(), Ok(SoundCmd::Reset | SoundCmd::Confirm)) {}
                        }
                        Err(_) => break,
                    }
                }
            }
        });
        Mutex::new(tx)
    })
}

pub fn start_beep_loop() {
    let _ = get_sender().lock().unwrap().send(SoundCmd::StartAlarm);
}

pub fn play_confirm_sound() {
    let _ = get_sender().lock().unwrap().send(SoundCmd::Confirm);
}

pub fn play_reset_sound() {
    let _ = get_sender().lock().unwrap().send(SoundCmd::Reset);
}
