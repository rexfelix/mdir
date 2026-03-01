# mdir - Modern Mdir CLI

MS-DOS 시절 Mdir 3.x의 사용자 경험을 현대 터미널 환경에서 재현한 TUI 파일 관리자입니다.

## 스크린샷 (예정)

## 주요 기능

- **멀티 컬럼 파일 목록** - 터미널 너비에 따라 1/2/3 컬럼 자동 전환
- **키보드 중심 탐색** - 방향키로 파일 탐색, Enter/Backspace로 디렉토리 이동
- **파일 선택(마킹)** - Space 키로 복수 파일 선택, 선택 파일 노란색 표시
- **파일 CRUD** - 복사(C), 이동(M), 삭제(D), 이름변경(R), 새 폴더(K)
- **상태바** - 현재 경로, 선택 파일 정보, 파일/디렉토리 수 표시
- **숨김 파일 토글** - H 키로 숨김 파일 표시/숨김 전환
- **안정적인 터미널 복원** - panic 발생 시에도 터미널 상태 복원

## 설치

### 요구사항

- Rust 1.85 이상
- macOS, Linux, Windows(WSL) 지원

### 빌드

```bash
git clone <repository-url>
cd mdir
cargo build --release
```

바이너리는 `target/release/mdir`에 생성됩니다.

### PATH에 추가 (선택)

```bash
cp target/release/mdir ~/.local/bin/
```

# 또는 처음부터

```bash
git clone <repository-url>
cd mdir
cargo install --path .
```

## 사용법

```bash
# 현재 디렉토리에서 실행
mdir

# 또는 cargo로 직접 실행
cargo run --release
```

### 단축키

| 키 | 기능 |
|---|------|
| `↑` `↓` `←` `→` | 커서 이동 (좌/우는 컬럼 간 이동) |
| `Enter` | 디렉토리 진입 |
| `Backspace` | 상위 디렉토리로 이동 |
| `Home` / `End` | 목록 처음/끝으로 이동 |
| `PageUp` / `PageDown` | 페이지 단위 이동 |
| `Space` | 파일 선택/해제 (복수 선택 가능) |
| `C` | 선택 파일 복사 |
| `M` | 선택 파일 이동 |
| `D` | 선택 파일 삭제 (확인 후) |
| `R` | 파일 이름 변경 |
| `K` | 새 디렉토리 생성 |
| `H` | 숨김 파일 표시 토글 |
| `Q` / `F10` | 종료 |
| `Ctrl+C` | 종료 |

### 컬럼 레이아웃

| 터미널 너비 | 컬럼 수 |
|------------|--------|
| 80 미만 | 1컬럼 |
| 80 ~ 119 | 2컬럼 |
| 120 이상 | 3컬럼 |

## 기술 스택

- **Rust** - 시스템 프로그래밍 언어
- **[Ratatui](https://github.com/ratatui/ratatui)** - TUI 프레임워크
- **[Crossterm](https://github.com/crossterm-rs/crossterm)** - 크로스 플랫폼 터미널 라이브러리

## 개발 로드맵

- [x] **Phase 1 (MVP)** - 파일 목록, 방향키 이동, 디렉토리 탐색, 상태바
- [x] **Phase 2** - CRUD 단축키 (Copy, Move, Delete, Rename, Mkdir)
- [ ] **Phase 3** - 파일 타입별 색상, 하단 정보창 고도화
- [ ] **Phase 4** - 내부 뷰어, 파일 검색

## 테스트

```bash
cargo test
```

현재 78개 단위 테스트가 포함되어 있습니다.

## 라이선스

MIT
