# maple-alarm 코드 개선 계획

## 배경

실제 사용 중 발견한 버그/이상 동작 목록:
- 알람 소리가 1초에 4번 (코드 기대치: 2번)
- 확인 버튼 시 소리 4번 (low, high, high, low) — Confirm + Reset 패턴으로 추정
- 초기화 버튼 시 소리 4번 (high, low, high, low) — Reset 2번 패턴으로 추정
- 버튼 따닥 클릭 시 중복 이벤트 가능성
- 플래시 피드백이 실제로 표시되지 않는 확실한 버그
- 깜빡임이 버튼 2개 교체 방식 → 중복 이벤트 위험 및 코드 복잡성

---

## 분석 결과

### 확인된 버그

**버그 A — on_confirm의 플래시 피드백 취소 (`src/ui.rs:261-267`)**

on_confirm에서 `btn_flash.set_visible(true)` → `refresh_ui()` 순서로 실행되는데,
refresh_ui()의 비알람 분기가 `btn_flash.set_visible(false)`를 덮어써 버림.
결과: 140ms 플래시 피드백이 실제로는 표시되지 않음.

**버그 B — 버튼 4개가 동일 좌표 (8, 80)에 겹침 (`src/ui.rs:113-159`)**

btn_normal, btn_alarm_light, btn_alarm_dark, btn_flash가 모두 같은 위치.
NWG Label은 실제 HWND 기반이므로 Z-order 최상위 widget이 마우스 이벤트를 독점해야 하지만,
set_visible 순서와 타이밍에 따라 예상치 못한 위젯이 이벤트를 받을 수 있음.
→ 소리 4번 버그의 유력한 원인.

**버그 C — on_reset 중복 실행 방지 없음 (`src/ui.rs:270-276`)**

빠르게 두 번 클릭하면 play_reset_sound()가 두 번 호출되어 소리가 연속 재생됨.

**잠재 이슈 D — on_confirm 후 refresh_ui에서 btn_normal 표시**

btn_flash가 표시될 간격(140ms) 동안 btn_normal도 같은 위치에 visible=true가 되면
두 위젯이 겹치거나 잘못된 이벤트를 받을 수 있음.

### 원인 불명 (로깅 필요)
- 알람 소리 정확한 재생 횟수 (코드상 500ms 주기 = 2번/초가 기대값)
- on_confirm/on_reset 이벤트 핸들러가 실제로 몇 번 호출되는지

---

## 개선 계획

### Step 1: 로깅 추가 (진단용)

`src/alarm.rs`와 `src/ui.rs`에 `eprintln!` 타임스탬프 로그 추가.
실행 후 stderr 출력으로 실제 호출 횟수와 타이밍 확인.

추가 위치:
- `alarm.rs`: alarming 루프 최상단, Confirm/Reset 분기 진입 시
- `ui.rs`: on_confirm, on_reset, on_blink 진입 시
- 포맷 예: `eprintln!("[{:?}] on_confirm called", std::time::Instant::now());`

### Step 2: 깜빡임 구조 개선 — 버튼 2개 → 1개 (`src/ui.rs`, `src/main.rs`)

**현재**: btn_alarm_light + btn_alarm_dark를 250ms마다 교체
**변경**: btn_alarm 하나만 두고, main.rs의 WM_CTLCOLORSTATIC 핸들러에서 blink_dark 상태를 읽어 배경색 결정

구체적 변경:
- `btn_alarm_light`, `btn_alarm_dark` → `btn_alarm` 하나로 통합 (필드 정의)
- `on_blink()`: 두 버튼 교체 대신 `blink_dark` 토글 후 `InvalidateRect`로 재그리기 요청
- `main.rs` WM_CTLCOLORSTATIC 핸들러: btn_alarm HWND 여부와 `blink_dark` 값으로 배경색 분기
- 관련 set_visible 호출들 정리

효과: 겹치는 버튼 1개 감소, 코드 단순화, 이벤트 중복 위험 감소.

### Step 3: on_confirm 플래시 버그 수정 (`src/ui.rs`)

`AlarmApp`에 `flash_active: RefCell<bool>` 필드 추가.

- `on_confirm`: `*self.flash_active.borrow_mut() = true` 설정 후 refresh_ui() 호출
- `refresh_ui()`의 비알람 분기:
  - `flash_active`가 true이면 btn_flash는 건드리지 않고 btn_normal도 숨김
  - false이면 기존 로직 (btn_normal=true, btn_flash=false)
- `on_confirm_flash_end`: `*self.flash_active.borrow_mut() = false` 후 btn_flash=false, btn_normal=true

### Step 4: on_reset 중복 클릭 방지 (`src/ui.rs`)

on_reset 진입 시 이미 reset_secs 상태면 early return:
```rust
if state.remaining_secs.load(Ordering::Acquire) == state.reset_secs { return; }
```

### Step 5: 로깅 결과 반영

Step 1의 로그를 확인한 후:
- 알람 소리가 실제로 4번이면 타이밍 재조정 (recv_timeout 값 변경 등)
- 이벤트 핸들러 중복 호출 확인 시 → guard 조건 보강 또는 버튼 구조 추가 정리

Step 5 완료 후 진단용 로그 제거.

---

## 수정 대상 파일

| 파일 | 관련 Step |
|------|-----------|
| `src/ui.rs` | Step 1, 2, 3, 4 |
| `src/alarm.rs` | Step 1, 5 |
| `src/main.rs` | Step 2 (WM_CTLCOLORSTATIC 핸들러) |

---

## 검증 방법

1. `cargo run` (DEBUG_TIMER 환경변수로 짧은 타이머 설정 가능) 후 stderr 로그 확인
2. 알람 발생 → 소리 횟수 카운트 (2번/초 기대)
3. 확인 버튼 클릭 → 소리 2번 (low+high), on_confirm 로그 1회
4. 초기화 버튼 클릭 → 소리 2번 (high+low), on_reset 로그 1회
5. 따닥 클릭 → 소리 1번만 (두 번째 클릭 무시)
6. 확인 클릭 후 140ms 이내 플래시(연한 빨강) 표시 확인
7. 깜빡임 시 단일 버튼 색상 변화로 동작 확인
