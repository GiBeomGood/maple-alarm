<img src="assets/icon.ico" width="64" alt="maple-alarm icon">

# maple-alarm

메이플스토리 **설치기 갱신 타이머** — 설치 파일 만료 전에 미리 알려주는 Windows 데스크탑 알람 앱.

## 개요

메이플스토리 설치기(런처)는 일정 시간이 지나면 갱신이 필요합니다. 이 앱은 카운트다운 타이머를 표시하고, 시간이 만료되기 전에 소리와 시각 효과(깜빡이는 테두리)로 사용자에게 알려줍니다.

## 기능

- 100초 카운트다운 타이머 (기본값)
- 타이머 만료 시 소리 알람 + 화면 테두리 깜빡임
- 항상 최상위(Always on Top) 창
- 트레이 아이콘 최소화 지원
- 싱글 인스턴스 보장 (중복 실행 방지)

## 다운로드

[Releases](../../releases) 페이지에서 `alarm-app.exe`를 다운받아 바로 실행할 수 있습니다.

## 빌드

```bash
cargo build --release
```

> Windows 전용 앱입니다 (`windows_subsystem = "windows"`).

## 기술 스택

- **Rust**
- **native-windows-gui (NWG)** — Win32 UI
- **windows-sys** — 저수준 Win32 API (GDI, DPI, WM_PAINT 등)
