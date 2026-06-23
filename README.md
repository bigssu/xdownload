# XDownloader

유튜브 mp3/mp4 다운로드 전용 **Windows 데스크톱 앱**.

사용자가 받아 자기 PC에서 실행하므로, yt-dlp가 **각자의 가정용 IP**로 동작한다 →
데이터센터 서버에서 발생하던 YouTube 봇 감지("Sign in to confirm you're not a bot")가
근본적으로 사라진다. Xtool 포털(https://xtool.duckdns.org)의 5번 항목으로 배포한다.

## 왜 데스크톱 앱인가

YouTube는 데이터센터 IP(클라우드 서버)를 봇으로 강하게 차단한다. 특히 음악·저작권
영상은 PO Token으로도 못 뚫는다. 반면 **사용자 PC의 가정용 IP**는 정상 사용자로 보여
봇 감지가 없다. 그래서 서버 웹앱이 아니라 "각 사용자 PC에서 실행되는 데스크톱 앱"이
유일하게 "무료 + 모든 영상 + 모든 사용자 자동"을 동시에 만족한다.

## 기능

- URL 입력 (여러 개 / 플레이리스트)
- 포맷: **Mp3** / **Mp4**
- 화질 선택: 최고 / 1080p / 720p
- 진행률 바
- 저장 폴더 지정 (기본: Downloads)
- 다운로드 히스토리 (로컬 저장)

## 구조

| 파일 | 역할 |
|------|------|
| `main.py` | 진입점 — wxPython App 실행 |
| `ui/main_frame.py` | 메인 창 (URL 입력·포맷/화질·진행률·히스토리) |
| `core/downloader.py` | yt-dlp 호출 + 진행률 파싱 (H.264 우선 format, mp4 remux) |
| `core/binaries.py` | yt-dlp/ffmpeg 바이너리 첫 실행 시 자동 다운로드·캐시 |
| `core/history.py` | 다운로드 히스토리 저장 (JSON) |
| `core/config.py` | 설정 (저장 폴더 등) |

## 다운로드 방식 (검증된 mazelines / Xcut ytdl.ts 방식)

- format(mp4): `bestvideo[height<=N][vcodec^=avc1]+bestaudio[ext=m4a]` 우선 →
  어느 플레이어에서나 재생되도록 H.264(avc1) 우선, 없으면 mp4 → best 폴백
- `--merge-output-format mp4 --remux-video mp4` (재인코딩 없이 mp4 보장)
- mp3: `bestaudio` → ffmpeg로 mp3 추출
- `--concurrent-fragments 4` (DASH/HLS 병렬 다운로드)

## 바이너리 자동 다운로드

`core/binaries.py`가 첫 실행 시 yt-dlp.exe와 ffmpeg.exe를 받아
`%LOCALAPPDATA%/XDownloader/bin/`에 캐시한다. yt-dlp는 자가 업데이트(`-U`)로 최신 유지.

## 개발

```bash
python -m venv .venv
.venv\Scripts\activate
pip install -r requirements.txt
python main.py
```

## 빌드 (배포용 exe)

```bash
pip install pyinstaller
pyinstaller --noconfirm --onefile --windowed --name XDownloader main.py
# 산출물: dist/XDownloader.exe  (yt-dlp/ffmpeg는 미포함 — 첫 실행 시 자동 다운로드)
```

빌드한 `XDownloader.exe`를 Oracle 서버에 올려 `https://xtool.duckdns.org/dl/XDownloader.exe`로
서빙하고, 포털 카드(5번)에서 받게 한다.

## 라이선스 / 주의

개인용 다운로드 보조 도구. YouTube 약관·저작권을 준수해 사용할 것.
