# Xdownload2

유튜브 다운로드와 로컬 미디어 변환을 하는 **Windows 데스크톱 앱** (Rust 단일 exe, 설치 불필요).
Python/wxPython 판 XDownloader v0.68 을 Rust 로 다시 만들고(1.0.0) 기능·성능을 넓혔다(2.0.0).
Python 원본은 `main` 브랜치 이력에 있다.

사용자가 자기 PC 에서 실행하므로 yt-dlp 가 **각자의 가정용 IP** 로 동작한다. 데이터센터 서버에서
생기던 YouTube 봇 감지가 생기지 않는다.

- 쓰는 사람용 안내: [docs/USER_GUIDE_KO.md](docs/USER_GUIDE_KO.md)
- 변경 내역: [CHANGELOG.md](CHANGELOG.md)
- 배포 준비 점검표: [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md)

## 기능

- **다운로드**: MP4(최고 / 4K / 1440p / 1080p / 720p / 480p), MP3, M4A. 자막 넣기, 썸네일·정보 넣기, 플레이리스트 전체.
- **작업 목록**: 항목별 진행률·속도·남은 시간, 취소·다시 시도·지우기. 동시에 1~5개(기본 3).
- **변환**: 동영상 → MP4(H.264, 크기·화질 선택) / MP3 / GIF, 이미지 → JPG / PNG / WebP. 원본은 그대로 두고 옆에 만든다.
- **속도**: 동시 다운로드, 그래픽카드 인코더(NVIDIA·Intel·AMD 자동, 실패 시 CPU), 이미지 동시 변환, 시작 시 도구 미리 준비.
- **편의**: 복사한 링크 자동으로 넣기, 파일·폴더 끌어다 놓기, 기록(파일 열기·다시 받기), 한국어/영어, 새 버전 알림.

## 구조

| 파일 | 역할 |
|------|------|
| `src/main.rs` | 진입점, `App`, 제목줄·아래 줄, 시작 준비 스레드, `--screenshot` |
| `src/tabs.rs` | 다운로드 / 변환 / 기록 / 설정 화면과 작업 목록 |
| `src/queue.rs` | 작업 큐(상태, 종류별 동시 실행 한도, 취소·재시도)와 작업 스레드 본체 |
| `src/downloader.rs` | yt-dlp 인자, 출력 줄 해석, 실패 시 업데이트 후 1회 재시도 |
| `src/converter.rs` | ffmpeg 변환(대상 수집, 출력 경로, 인코더 선택, 진행률) |
| `src/binaries.rs` | yt-dlp / ffmpeg 최초 다운로드, SHA256 검증, 캐시 |
| `src/proc.rs` | 창 없는 자식 프로세스 실행, 줄 단위 출력, 프로세스 트리 취소 |
| `src/update.rs` | 앱 새 버전 확인, 클립보드 링크 찾기 |
| `src/shell.rs` | 파일·폴더·링크 열기(Windows 셸 API) |
| `src/settings.rs` | 설정 · 기록 JSON |
| `src/i18n.rs` | 한/영 문자열 표 |
| `src/crashlog.rs` | 패닉 로그 |
| `src/ui/` | 디자인 토큰(`theme`), 공용 위젯(`widgets`), 아이콘(`icons`) |
| `packaging/` | `release.ps1`(배포 파일), `ui_smoke.ps1`(실제 창 검증), `notices.py`(오픈소스 고지) |

데이터 위치: `%APPDATA%\Xdownload2\` — `settings.json`, `history.json`, `bin\`(yt-dlp.exe, ffmpeg.exe), `logs\crash.log`.

## 동작 요약

- mp4: 1080p 까지는 `bestvideo[height<=N][vcodec^=avc1]+bestaudio[ext=m4a]` 우선(어디서나 재생되는 H.264).
  1080p 보다 높게 고르면 `bestvideo[height>1080][height<=N]` 을 먼저 고른다(유튜브에 4K H.264 는 없다).
  `--merge-output-format mp4 --remux-video mp4` 로 재인코딩 없이 mp4.
- 실패하면 `yt-dlp -U` 로 업데이트한 뒤 한 번 더 시도한다(업데이트는 실행당 1회). 시작할 때도 하루 한 번 미리 업데이트한다.
- yt-dlp / ffmpeg 는 exe 에 넣지 않고 처음 필요할 때 받아 SHA256 을 검증한다.
- 동시 실행 한도: 다운로드 = 설정값, 동영상 변환 = CPU 1 / GPU 2, 이미지 변환 = 코어 수(최대 8).
- 취소는 프로세스 트리 전체를 끝낸다(yt-dlp 가 띄운 ffmpeg 포함).

## 개발

Git Bash 에서는 `export PATH="$HOME/.cargo/bin:$PATH"` 가 필요하다.

```bash
cargo run                 # 실행
cargo test                # 단위 테스트
cargo run -- https://youtu.be/… D:\clip.mov   # 링크는 입력칸에, 파일은 변환 목록에
```

화면 확인 (창을 띄워 한 장 저장하고 종료, 사용자 설정은 건드리지 않음):

```bash
cargo run -- --screenshot target/shots/download.png --tab download
cargo run -- --screenshot target/shots/convert_en.png --tab convert --lang en   # history, settings 도 가능
```

실제 네트워크로 끝까지 확인 (동시 다운로드, 포맷, GPU·CPU 변환):

```bash
cargo test --release live -- --ignored --nocapture --test-threads 1
```

## 배포

```powershell
pwsh packaging\release.ps1     # 테스트 → release 빌드 → dist\ (exe, sha256, 버전 json)
pwsh packaging\ui_smoke.ps1    # 만든 exe 를 실제로 띄워 눌러 본다
```

순서와 남은 일은 [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md) 에 있다.

## 라이선스 / 주의

개인용 다운로드 보조 도구. YouTube 약관·저작권을 준수해 사용할 것.
쓰는 오픈소스는 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
