---
name: XDownloader
description: wxPython 4.2 기반 YouTube 다운로더 데스크톱 앱의 디자인 시스템 (XChange 파생, 다크/라이트)
tokens:
  access: 'c("key") / f(delta, bold) / dip(px) / PAD_XS·PAD_S·PAD_M'
  hardcoding: forbidden
spacing:
  PAD_XS: 4
  PAD_S: 8
  PAD_M: 14
typography:
  base: 'Segoe UI 10pt (FONT_BASE), f(delta, bold)로 가감'
colors_dark:
  bg: "#13121F"
  card: "#21203A"
  surface: "#2F2E50"
  border_card: "#302F52"
  accent: "#0078D4"
  text: "#E6E4FF"
  text_dim: "#7F7DAA"
  ok: "#4EC976"
  err: "#F06969"
  on_fill: "#FFFFFF"
colors_light:
  bg: "#F4F3FA"
  card: "#FFFFFF"
  surface: "#EAE8F5"
  border_card: "#DAD7EC"
  accent: "#0078D4"
  text: "#1E1D2E"
  text_dim: "#6D6B8C"
  ok: "#17803F"
  err: "#B42D3A"
  on_fill: "#FFFFFF"
---

## Overview

XDownloader의 UI는 자매 프로젝트 **XChange의 디자인 시스템을 이식**한 것이다.
딥 퍼플-틴트 다크 카드 미학 + Segoe UI 타이포 + 라운드 커스텀 위젯을 쓰며,
라이트/다크 두 테마를 모두 지원한다(다크=XChange 원본 값, 라이트=파생 값).

**방식(8원칙, 출처: XChange `UI_UX_RULES`):** 색·폰트·px 하드코딩 금지 —
전부 디자인 토큰(`c()/f()/dip()/PAD_*`)으로만 접근한다. 레이아웃은 Sizer 전용,
커스텀 위젯은 `GraphicsContext`로 직접 그린다. 관련 모듈: `ui/theme_tokens.py`(토큰),
`ui/widgets.py`(공용 위젯).

> 과거 "OS 시스템 색을 따른다" 원칙(v0.67 이하)은 이 토큰 시스템으로 대체되었다.

## Design Tokens (`ui/theme_tokens.py`)

- **색:** `C_DARK`/`C_LIGHT` 딕셔너리(동일 키 집합) → `c("key")`가 현재 테마 색을 반환.
  `set_theme("dark"|"light")`로 전환(색 캐시 자동 무효화). 대비는 WCAG AA로 검증됨
  (`tests/test_theme_tokens.py`가 회귀로 고정).
- **폰트:** `FONT_BASE = 10`(Segoe UI). `f(delta, bold)`로 크기·굵기 가감.
- **DPI:** 모든 px는 `dip(px)` 통과(`wx.Window.FromDIP`).
- **아이콘 폰트:** `icon_font(size)` — Windows 내장 **Segoe MDL2 Assets**(벡터, 애셋 추가 0).
- **간격:** `PAD_XS=4`(밀접), `PAD_S=8`(내부), `PAD_M=14`(섹션/거터).

## Colors

라이트/다크 모두 XChange 토큰 구조를 공유한다. accent(#0078D4)·danger는 두 테마 공용,
표면/텍스트는 테마별로 반전. 상태 텍스트용 `ok`/`err`, 채움형 버튼 라벨용 `on_fill`(흰색)은
XDownloader 확장 토큰이다. 전체 표는 frontmatter 참조.

## Typography

기본 **Segoe UI 10pt**. 섹션 헤더(`SectionCard` 제목)와 강조는 `f(0, bold=True)`.
네이티브 데스크톱 관례상 크기보다 굵기·색으로 위계를 준다.

## Components (`ui/widgets.py`)

- **SectionCard:** 라운드 테두리 카드(헤더 + `body` sizer). 포맷·화질·로컬 파일 변환·히스토리 섹션.
- **FlatButton:** 커스텀 페인트 버튼(비트맵 캐시). `btn_style`=accent/secondary/danger.
  **아이콘 버튼** — 라벨 대신 Segoe MDL2 글리프를 그리고, 라벨은 **호버 툴팁(이름+기능)** 으로 노출.
  `badge`가 있으면 메인 아이콘 왼쪽에 파일/폴더 구분 배지를 나란히 그린다. accent/danger 라벨색은 `on_fill`.
- **DarkChoice:** 다크 테마 드롭다운(`OwnerDrawnComboBox`). 화질 선택.
- **ToggleSwitch:** on/off 스위치(현재 미사용, 이식 완비).
- **_ProgressBar:** 커스텀 진행바(네이티브 게이지의 다크 트랙 문제 회피). 색은 `c("surface")`/`c("accent")`.

### 아이콘(Segoe MDL2 Assets)

다운로드↓(E896)/취소✕(E711), 폴더 변경(ED25), 포맷 MP4 필름(E8B2)/MP3 음표(E8D6),
변환 = 동영상(E714)/이미지(E91B) + 파일(E7C3)·폴더(ED25) 배지. 모든 버튼에 호버 툴팁.

## Theme / Language / Window

- **테마 토글(☾/☀):** `set_theme` + 패널 재생성(입력·선택값 보존). 다크↔라이트 즉시 전환.
- **언어 토글(한/En):** ko/en, 패널 재생성. 문자열은 `core/i18n.py`.
- **창 크기:** 기본은 콘텐츠(모든 버튼 + 히스토리 2~3줄)에 맞춰 열고, 사용자가 기본의
  60%(=40% 작게)까지 축소 가능.

## Localization

`core/i18n.py`의 `{lang: {key: text}}` 테이블 + `tr(lang, key)`. 누락 키는 한국어 → 키 자체로 폴백.
아이콘 버튼의 툴팁도 이 라벨을 재사용한다.
