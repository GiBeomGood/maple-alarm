# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**maple-alarm**은 Windows 전용 데스크탑 타이머 알람 앱으로, MapleStory 게임 런처/설치 관리자 새로고침 타이머(기본 105초)가 만료될 때 시각·음성으로 알림을 주는 Rust 프로그램이다. 바이너리 이름은 `timer.exe`.

## Build & Run

```bash
# 릴리즈 빌드 (출력: ./target/release/timer.exe)
cargo build --release

# 초 수를 인자로 지정해 실행
timer.exe --seconds 50
timer.exe 60
```

테스트 및 린트 명령은 별도 정의 없음. 기본 `cargo check` / `cargo clippy`로 정적 검사.

## Release

`v*` 태그 push 시 `.github/workflows/release.yml`이 자동으로 Windows 릴리즈 빌드 후 GitHub 릴리즈 draft를 생성한다.

## Architecture

공유 상태는 **`src/state.rs`의 `SharedState`** 구조체 하나에 집중되어 있으며, 전부 원자 타입(`AtomicU64`, `AtomicBool`, `AtomicU32`)이라 뮤텍스 없이 스레드 간 공유한다.

### 모듈별 역할

| 모듈 | 역할 |
|------|------|
| `src/main.rs` | NWG 초기화, DPI 스케일링, 창 위치 지정(화면 우하단), 커스텀 Win32 메시지 핸들러(WM_PAINT 경계선, WM_CTLCOLORSTATIC, 볼륨 슬라이더 드래그) |
| `src/ui.rs` | `AlarmApp` NWG 구조체 정의, 모든 UI 컨트롤 및 이벤트 핸들러(`on_tick`, `on_confirm`, `on_reset`, `on_blink`), 트레이 아이콘, 창 드래그 |
| `src/state.rs` | 전역 공유 상태(`SharedState`) - 잔여 초, 알람 활성 여부, 볼륨 등 |
| `src/timer.rs` | 백그라운드 카운트다운 스레드 (1초 sleep 루프, remaining_secs 감소) |
| `src/alarm.rs` | 사인파 PCM 사전 생성(OnceLock), MPSC 채널로 사운드 명령 수신, Win32 `PlaySoundA` 직접 호출 |
| `src/instance.rs` | 단일 인스턴스 강제 (전역 뮤텍스 `Global\AlarmAppSingletonMutex_v1`) |

### 데이터 흐름

1. 타이머 스레드 → `remaining_secs` 원자 감소 → `nwg::Notice`로 UI 스레드에 알림
2. UI 스레드 `on_tick()` → `refresh_ui()` 호출 → 레이블·버튼 가시성 업데이트
3. `remaining_secs == 0` 도달 시 → `alarm_active = true` + 사운드 스레드에 `StartAlarm` 명령
4. 사용자 확인/초기화 버튼 클릭 → 사운드 스레드에 `Confirm`/`Reset` 명령 → 피드백 음 재생

### 커스텀 그리기

NWG 기본 페인트를 우회해 `main.rs`에서 직접 Win32 메시지를 가로채어 처리:
- **경계선**: `WM_PAINT` — 알람 상태에 따라 빨강/회색 2px 경계선
- **버튼**: 커스텀 텍스트·배경 렌더링
- **볼륨 슬라이더**: `WM_MOVING`으로 메인 창과 동기화된 위치 유지

## Key Technical Constraints

- **Windows 전용**: `windows-sys`, `native-windows-gui`, Win32 API 직접 사용
- **원자 상태만 사용**: 데드락 없음, 뮤텍스 대신 atomic 타입으로 공유 상태 관리
- **UI 스레드**: NWG는 단일 UI 스레드 모델이므로 UI 조작은 반드시 UI 스레드에서만
- **릴리즈 최적화**: `opt-level = "z"`, LTO, `panic = "abort"`, 심볼 strip 적용
- **빌드 스크립트**: `build.rs`가 `assets/app.manifest`를 실행 파일에 삽입 (DPI 인식, Windows 10+ 호환)
