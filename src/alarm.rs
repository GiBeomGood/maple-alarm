use std::sync::OnceLock;
use std::sync::Mutex;
use std::sync::mpsc::{self, Sender, RecvTimeoutError};
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;
use std::f32::consts::TAU;

extern "system" {
    fn waveOutSetVolume(hwo: usize, dwvolume: u32) -> u32;
    fn PlaySoundA(pszsound: *const u8, hmod: usize, fsound: u32) -> i32;
}

const SND_SYNC:   u32 = 0x0000;
const SND_MEMORY: u32 = 0x0004;

static CURRENT_VOLUME: AtomicU32 = AtomicU32::new(100);

pub fn set_volume(v: u32) {
    CURRENT_VOLUME.store(v, Ordering::Relaxed);
}

fn apply_volume() {
    let vol = CURRENT_VOLUME.load(Ordering::Relaxed);
    let level = (vol as u64 * 0xFFFF / 200) as u16 as u32;
    let packed = (level << 16) | level;
    unsafe { waveOutSetVolume(0, packed); }
}

// Generates a PCM square-wave WAV buffer (44100 Hz, mono, 16-bit).
// Called once per frequency via OnceLock; the buffer is reused forever.
fn make_wav(freq: u32, duration_ms: u32) -> Vec<u8> {
    const SAMPLE_RATE: u32 = 44100;
    let num_samples = (SAMPLE_RATE * duration_ms / 1000) as usize;
    let samples_per_half = (SAMPLE_RATE / (freq * 2)) as usize;
    let data_size = num_samples * 2;

    let mut buf = Vec::with_capacity(44 + data_size);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&((36 + data_size) as u32).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());          // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes());           // PCM
    buf.extend_from_slice(&1u16.to_le_bytes());           // mono
    buf.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    buf.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    buf.extend_from_slice(&2u16.to_le_bytes());           // block align
    buf.extend_from_slice(&16u16.to_le_bytes());          // bits per sample
    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&(data_size as u32).to_le_bytes());

    let _ = samples_per_half; // unused after switching to sine
    for i in 0..num_samples {
        let t = i as f32 / SAMPLE_RATE as f32;
        let sample = (f32::sin(TAU * freq as f32 * t) * 16000.0) as i16;
        buf.extend_from_slice(&sample.to_le_bytes());
    }

    buf
}

fn play_wav(buf: &[u8]) {
    unsafe { PlaySoundA(buf.as_ptr(), 0, SND_MEMORY | SND_SYNC); }
}

// Pre-built WAV buffers — generated on first use, reused on every subsequent call.
static WAV_1200_100: OnceLock<Vec<u8>> = OnceLock::new();
static WAV_1800_80:  OnceLock<Vec<u8>> = OnceLock::new();
static WAV_2400_80:  OnceLock<Vec<u8>> = OnceLock::new();

fn wav_1200_100() -> &'static [u8] { WAV_1200_100.get_or_init(|| make_wav(1200, 100)) }
fn wav_1800_80()  -> &'static [u8] { WAV_1800_80 .get_or_init(|| make_wav(1800,  80)) }
fn wav_2400_80()  -> &'static [u8] { WAV_2400_80 .get_or_init(|| make_wav(2400,  80)) }

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
                    apply_volume();
                    play_wav(wav_1200_100());
                    match rx.recv_timeout(Duration::from_millis(400)) {
                        Ok(SoundCmd::Confirm) => {
                            alarming = false;
                            apply_volume();
                            play_wav(wav_1800_80()); play_wav(wav_2400_80());
                            while matches!(rx.try_recv(), Ok(SoundCmd::Reset | SoundCmd::Confirm)) {}
                        }
                        Ok(SoundCmd::Reset) => {
                            alarming = false;
                            apply_volume();
                            play_wav(wav_2400_80()); play_wav(wav_1800_80());
                            while matches!(rx.try_recv(), Ok(SoundCmd::Reset | SoundCmd::Confirm)) {}
                        }
                        Ok(SoundCmd::StartAlarm) | Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                } else {
                    match rx.recv() {
                        Ok(SoundCmd::StartAlarm) => { alarming = true; }
                        Ok(SoundCmd::Confirm) => {
                            apply_volume();
                            play_wav(wav_1800_80()); play_wav(wav_2400_80());
                            while matches!(rx.try_recv(), Ok(SoundCmd::Reset | SoundCmd::Confirm)) {}
                        }
                        Ok(SoundCmd::Reset) => {
                            apply_volume();
                            play_wav(wav_2400_80()); play_wav(wav_1800_80());
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
