# Xdownload2

유튜브 MP3/MP4 다운로드와 로컬 미디어 변환을 하는 **Windows 데스크톱 앱** (Rust 단일 exe).
Python/wxPython 판 XDownloader v0.68 을 Rust 로 다시 만든 것이다. Python 원본은 `main` 브랜치 이력에 있다.

사용자가 자기 PC 에서 실행하므로 yt-dlp 가 **각자의 가정용 IP** 로 동작한다. 데이터센터 서버에서
생기던 YouTube 봇 감지("Sign in to confirm you're not a bot")가 생기지 않는다.

## 기능

- URL 입력 (여러 개는 줄바꿈, 플레이리스트 가능)
- 포맷 **MP4** / **MP3**, 화질 최고 / 1080p / 720p
- 진행률 표시, 취소(버튼 · Esc · 창 닫기), 입력창 밖에서 Enter 로 시작
- 저장 폴더 지정 (기본: Downloads)
- 다운로드 히스토리 (최근 100개, 두 번 클릭하면 폴더 열기)
- 로컬 동영상 변환: 파일/폴더 → MP4 / H.264 / AAC / 1080p 이하
- 로컬 이미지 변환: 파일/폴더 → JPG
- 한국어 / 영어 전환 (제목줄, 재시작 불필요)

로컬 변환은 원본을 그대로 두고 원본 옆에 새 파일을 만든다. 폴더 선택은 그 폴더 바로 아래 파일만 처리한다.

## 구조

| 파일 | 역할 |
|------|------|
| `src/main.rs` | 진입점, 화면, 작업 스레드 연결, `--screenshot` |
| `src/downloader.rs` | yt-dlp 인자 · 진행률 파싱 · 실패 시 업데이트 후 1회 재시도 |
| `src/converter.rs` | ffmpeg 로컬 변환 (대상 수집, 출력 경로, 진행률) |
| `src/binaries.rs` | yt-dlp / ffmpeg 최초 실행 시 다운로드 · SHA256 검증 · 캐시 |
| `src/proc.rs` | 창 없는 자식 프로세스 실행, 줄 단위 출력, 프로세스 트리 취소 |
| `src/settings.rs` | 설정 · 히스토리 JSON |
| `src/i18n.rs` | 한/영 문자열 표 |
| `src/crashlog.rs` | 패닉 로그 |
| `src/ui/` | 디자인 토큰(`theme`), 공용 위젯(`widgets`), 아이콘(`icons`) |

데이터 위치: `%APPDATA%\Xdownload2\` — `settings.json`, `history.json`, `bin\`(yt-dlp.exe, ffmpeg.exe), `logs\crash.log`.

## 다운로드 방식

- mp4: `bestvideo[height<=N][vcodec^=avc1]+bestaudio[ext=m4a]` 우선 → 어느 플레이어에서나 재생되도록
  H.264(avc1) 우선, 없으면 mp4 → best 폴백
- `--merge-output-format mp4 --remux-video mp4` (재인코딩 없이 mp4 보장)
- mp3: `bestaudio` → ffmpeg 로 mp3 추출
- `--concurrent-fragments 4`
- 실패하면 `yt-dlp -U` 로 업데이트한 뒤 한 번 더 시도한다.

## 바이너리 자동 다운로드

exe 에 yt-dlp / ffmpeg 를 넣지 않는다. 처음 필요할 때 받아 `%APPDATA%\Xdownload2\bin\` 에 둔다.
같은 릴리스가 게시한 SHA256 목록으로 검증하고, 검증을 통과한 파일만 제자리에 놓는다.
로컬 변환만 쓰면 ffmpeg 만 받는다.

## 개발

Git Bash 에서는 `export PATH="$HOME/.cargo/bin:$PATH"` 가 필요하다.

```bash
cargo run                 # 실행
cargo test                # 단위 테스트
cargo build --release     # 배포용: target/release/xdownload2.exe
```

화면 확인 (창을 띄워 한 장 저장하고 종료, 사용자 설정은 건드리지 않음):

```bash
target/release/xdownload2.exe --screenshot target/shots/ko.png
target/release/xdownload2.exe --screenshot target/shots/en.png --lang en
```

실제 네트워크로 끝까지 확인 (바이너리 준비 → MP4/MP3 다운로드 → 변환):

```bash
cargo test --release live_download_and_conversion -- --ignored --nocapture
```

## 라이선스 / 주의

개인용 다운로드 보조 도구. YouTube 약관·저작권을 준수해 사용할 것.
