# RUST_PORT — XDownloader(Python) → Xdownload2(Rust) 재작성 계획 (2026-11-25, Review-Tier: T1)

목표: wxPython 앱 XDownloader v0.68 의 모든 기능을 Rust 단일 exe `xdownload2.exe` 로 다시 만든다.
방식은 자매 프로젝트 Xgif2(`C:\_NOAV\Xgif`, Python XGif → Rust)와 같게 한다.

## 구조 사실 (실측)
- 원본: Python 약 1,900줄. `core/`(config, history, binaries, downloader, converter, i18n, winapi), `ui/`(theme_tokens, widgets, main_frame), 테스트 3개.
- Xgif2 방식: Rust 2021 + eframe 0.31(egui), Windows 전용, `#![windows_subsystem = "windows"]`, 정적 `tr()` 한/영 표(AtomicU8 언어),
  serde JSON 설정(`#[serde(default)]`), `crashlog` 패닉 훅, `build.rs`(winresource 아이콘 + 빌드 시각), 시스템 글꼴(맑은 고딕·Segoe UI) + egui-phosphor 아이콘,
  장식 없는 투명 창 + 직접 그린 제목줄·크기 조절 영역, 모듈 안 `#[cfg(test)]` 단위 테스트(TDD), `--screenshot <png>` 화면 검사, `docs/plans` 계획·보고서.
- 빌드: Git Bash 에서 `export PATH="$HOME/.cargo/bin:$PATH"`, cargo 1.100.

## 운영자 확정
1. Python 코드는 `rust-port` 브랜치에서 삭제(원본은 main·git 이력에 남음).
2. UI 는 Xgif2 스타일(다크 전용 토큰, 직접 그린 제목줄). 라이트 테마·테마 전환은 없앤다.
3. 새 이름 **Xdownload2**: exe `xdownload2.exe`, 데이터 `%APPDATA%\Xdownload2\`(settings.json, history.json, bin\, logs\). 기존 XDownloader 설정은 이어받지 않는다.
4. `rust-port` 브랜치에 커밋, push 안 함.

## 모듈
| 파일 | 역할 | 원본 |
|---|---|---|
| `src/main.rs` | 진입점, `App`, 화면, 작업 스레드 연결, `--screenshot` | main.py, ui/main_frame.py |
| `src/settings.rs` | 설정 + 히스토리(최대 100) JSON, 임시파일 후 교체 저장 | core/config.py, core/history.py |
| `src/i18n.rs` | `(key, ko, en)` 정적 표, `tr`, `{i}` 치환 `fill` | core/i18n.py |
| `src/proc.rs` | 창 없는 자식 프로세스 실행, stdout+stderr 줄 단위 수신, 취소·강제 종료 | downloader/converter 의 Popen 공통부 |
| `src/binaries.rs` | yt-dlp.exe / ffmpeg.exe 최초 다운로드, SHA256 검증, `.part` 후 교체, zip 에서 `bin/ffmpeg.exe` 추출 | core/binaries.py |
| `src/downloader.rs` | yt-dlp 인자, 진행률·파일명 파싱, 실패 시 `-U` 후 1회 재시도 | core/downloader.py |
| `src/converter.rs` | 대상 수집, 출력 경로, ffmpeg 인자, 진행률 파싱 | core/converter.py |
| `src/crashlog.rs` | 패닉 로그 | Xgif2 그대로 |
| `src/ui/{theme,widgets,icons}.rs` | Xgif2 토큰·버튼·말풍선·아이콘 중 쓰는 것만 | Xgif2 |

새 크레이트: `ureq`(native-tls = Windows SChannel), `sha2`, `zip`(deflate). 정규식은 쓰지 않고 문자열 파싱.

## 동작 (원본과 같게)
- URL 여러 줄 → 순서대로 다운로드. 포맷 MP4/MP3, 화질 최고/1080p/720p(MP3 일 때 비활성).
- yt-dlp 인자·format 문자열·`--concurrent-fragments 4`·`--merge-output-format mp4 --remux-video mp4`·MP3 추출 옵션 동일. 추가: `--encoding utf-8`(한글 파일명 표시용).
- 실패(0·취소 아님) 시 `yt-dlp -U` 후 1회 재시도. 성공 시 히스토리 기록. 끝나면 성공/실패 건수 표시.
- 로컬 변환: 동영상 → MP4/H.264/AAC/1080p 이하, 이미지 → JPG. 폴더는 바로 아래 파일만, `_h264_1080p` 산출물 재입력 제외, 출력 이름 `-2..` 로 유일화, `.jpg` 원본은 건너뜀.
- 바이너리: 릴리스의 SHA256 목록에서 파일명이 정확히 일치하는 줄로 검증. 불일치면 `.part` 삭제 후 오류.
- 취소: 버튼 / Esc / 창 닫기 → 플래그 + 자식 프로세스 종료. 작업 중 언어 전환 가능(egui 는 매 프레임 다시 그림).
- Enter(입력창 밖) = 다운로드 시작. 히스토리 두 번 클릭 = 폴더 열기.
- 원본의 "링크를 입력하세요" 메시지 상자는 상태줄 표시로 바꾼다.

## 제외
라이트 테마, OS 다크 감지·Win32 다크 보정(다크 전용이라 불필요), MSIX 패키징(스토어 배포 대상 아님), 설정 창.

## 테스트 (TDD — 순수 로직은 테스트 먼저)
- converter: 원본 테스트 5개 이식 + 진행률 줄 파싱(`Duration:`, `out_time=`).
- downloader: mp4/mp3 인자, 화질→높이, `[download] 42.3%` 파싱, `Destination:` 파일명.
- binaries: 체크섬 목록에서 정확 일치 줄 찾기(`-shared` 변형 혼동 금지), sha256 알려진 값, 검증 실패 시 파일 삭제, zip 에서 ffmpeg.exe 추출.
- settings: 빈/구버전 JSON 기본값, 왕복, 히스토리 100개 상한·최신순.
- i18n: 모든 키 ko/en 비어 있지 않음·키 중복 없음, 화면에서 쓰는 키 전부 존재, `fill` 치환.
- proc: `cmd /c echo` 줄 수신·종료코드, 취소 시 -1.
- ui: 쓰는 아이콘 이름 전부 글리프 존재, 말풍선 위치.

## 수용 기준
- `cargo build` 경고 0, `cargo test` 전부 통과, `cargo build --release` 성공.
- `--screenshot` 으로 한국어·영어 화면 PNG 확인.
- 실제 실행: 바이너리 자동 준비 → 짧은 영상 1개 MP4 다운로드 성공, 로컬 이미지 1개 JPG 변환 성공(오케스트레이터가 직접).

## 계획검토 반영 (Codex sol, 2026-11-25, REVISE → 반영 후 재검토 없이 진행)
1~3. 프로세스: stdout·stderr 를 각각 스레드로 비우고(교착 방지) 줄을 채널로 모은다. 취소는 읽기와 무관하게 `taskkill /T /F` 로 프로세스 트리 전체를 끝낸다(yt-dlp 가 띄운 ffmpeg 포함). 시작 중 들어온 취소도 놓치지 않는다. 손자 프로세스 취소 테스트 포함.
4. 업데이트는 같은 Job 아래에서 돌아 취소로 멈춘다. 실패해도 기존 바이너리로 1회 재시도, 취소 후에는 재시도하지 않는다(`retry_once` 테스트). 120초 제한은 두지 않았다 — 취소로 대신한다.
5. 출력은 바이트로 읽어 손실 허용 디코딩한다.
6~7. ureq Agent 에 NativeTls + 플랫폼 인증서 저장소를 설정하고 연결 30초·응답 60초 제한. 본문 전체 제한은 두지 않는다(큰 파일). HTTP 오류는 실패로 처리, 실패한 `.part` 는 지운다.
8. 설정·히스토리는 같은 폴더의 임시파일에 쓰고 rename 으로 교체한다.
9. zip 은 디스크로 받아 검증한 뒤 `…/bin/ffmpeg.exe` 만 `.part` 로 풀어 교체한다.
10. 산출물 제외는 폴더 탐색에만 적용, 직접 고른 파일은 허용(테스트 포함).
12. 작업자는 채널로 알리고 다시 그리기를 요청한다. 옵션은 시작할 때 고정. 상태줄은 키+값으로 보관해 언어 전환을 따른다.
13. 실행 검증을 넓혔다: MP4, MP3, 실패 링크가 섞인 배치(계속 진행·집계·성공분만 기록), PNG→JPG, MP4 변환 — `live_download_and_conversion`(수동, `--ignored`).

## 최종 상태 (2026-11-25)
- `cargo build` 경고 0, `cargo test` 35 통과(+수동 1), release `target/release/xdownload2.exe` 7.5 MB.
- `--screenshot` 한국어·영어 화면 확인.
- 수동 검증 통과: yt-dlp/ffmpeg 자동 준비(SHA256 검증), MP4 720p, MP3, 실패 1 + 성공 1 배치("완료 — 성공 1, 실패 1", 업데이트 후 재시도 동작), PNG→JPG, MP4→`_h264_1080p.mp4`.
- 미검증: 창을 직접 눌러 보는 조작(파일 선택 창, 취소 버튼·Esc, 창 크기 조절, 히스토리 두 번 클릭)은 자동화하지 않았다. 취소·집계·상태 전환은 단위 테스트로만 확인했다.

## 코드 리뷰 (Codex sol high, 2026-11-25, 1회) — High 없음
- Medium: 바이너리 다운로드가 중간에 멈추면 취소가 끝나지 않음 → 읽기마다 60초 제한(`timeout_per_read`) 추가.
- Medium: 창을 두 개 띄우면 나중 창이 먼저 창의 히스토리를 덮어씀 → 기록할 때 파일을 다시 읽고 추가.
