---
name: XDownloader
description: wxPython 4.2 기반 YouTube 다운로더 데스크톱 앱의 디자인 토큰 (Windows 네이티브)
colors:
  primary: "#000000"
  surface: "#F0F0F0"
  surface-input: "#FFFFFF"
  disabled: "#6D6D6D"
  accent: "#0078D7"
  success: "#008000"
  error: "#B40000"
typography:
  body:
    fontFamily: "맑은 고딕"
    fontSize: 12px
  button-primary:
    fontFamily: "맑은 고딕"
    fontSize: 12px
    fontWeight: 700
spacing:
  tight: 4px
  inner: 6px
  section: 12px
components:
  url-input:
    backgroundColor: "{colors.surface-input}"
    textColor: "{colors.primary}"
  status-success:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.success}"
  status-error:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.error}"
  status-disabled:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.disabled}"
  button-download:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.primary}"
    typography: "{typography.button-primary}"
    height: 40px
---

## Overview

XDownloader는 wxPython 네이티브 위젯으로 만든 Windows 데스크톱 유틸리티다.
독자적 브랜드 팔레트를 강요하지 않고 **OS 시스템 색을 그대로 따르는 것**을 원칙으로
한다. 커스텀 색은 "성공/실패" 같은 의미 전달이 꼭 필요한 곳에만 최소한으로 쓴다.
따라서 이 문서의 토큰 대부분은 Windows 시스템 색의 실제 값을 기록한 것이다.

라이트/다크 테마와 한국어/영어 언어는 창 상단 우측 토글(`한`/`En`, 다크 아이콘
`☾`/`☀`)에서 즉시 전환하며 config에 저장된다.
라이트 테마는 위 OS 시스템 색을 그대로 쓰고, 다크 테마만 아래 고정 팔레트를 적용한다.
테마 미설정(최초 실행) 시에는 OS 다크모드 설정을 따라 초기값을 정한다.

## Colors

- **primary (#000000):** 기본 텍스트색(시스템 WINDOWTEXT). 라벨·입력·버튼 글자.
- **surface (#F0F0F0):** 패널 배경(시스템 BTNFACE). 라벨·상태줄·버튼이 올라가는 면.
- **surface-input (#FFFFFF):** 텍스트 입력창 배경(시스템 WINDOW).
- **disabled (#6D6D6D):** 비활성 텍스트(시스템 GRAYTEXT).
- **accent (#0078D7):** 기본 버튼(다운로드) 포커스 강조 테두리(시스템 HIGHLIGHT). OS가 그린다.
- **success (#008000):** 다운로드 완료 상태 메시지. `core` 코드의 `wx.Colour(0,128,0)`.
- **error (#B40000):** 실패/오류 상태 메시지. `wx.Colour(180,0,0)`.

### 다크 테마 팔레트 (다크 모드에서만 적용)

- **bg (#1E1E1E):** 패널 배경. `wx.Colour(30,30,30)`.
- **input-bg (#2D2D30):** 입력창·리스트·드롭다운 배경. `wx.Colour(45,45,48)`.
- **fg (#DCDCDC):** 기본 텍스트색. `wx.Colour(220,220,220)`.
- **success-dark (#4EC976):** 다크 배경용 밝은 성공색. `wx.Colour(78,201,118)`.
- **error-dark (#F06969):** 다크 배경용 밝은 실패색. `wx.Colour(240,105,105)`.

> wxWidgets 3.2엔 네이티브 다크 API가 없어, 위젯 색칠만으로는 타이틀바·스크롤바·
> 드롭다운·게이지가 흰색으로 남는다. 이를 다음으로 보강해 **완전 다크**를 구현한다:
> - `core/winapi.py` — Win32(uxtheme/dwmapi) 직접 호출: 앱 다크모드(`SetPreferredAppMode`),
>   타이틀바(`DwmSetWindowAttribute` + `EVT_SHOW` 재적용·NC 리드로우), 스크롤바·드롭다운
>   (`SetWindowTheme` `DarkMode_Explorer`/`DarkMode_CFD`).
> - 진행바는 네이티브 게이지 대신 직접 그리는 `_ProgressBar`(wx.Panel)로 대체.
> - 포맷은 RadioBox 대신 RadioButton을 써 항목 텍스트 색이 다크에서도 보이게 함.

## Typography

기본 폰트는 시스템 GUI 폰트인 **맑은 고딕 9pt**(약 12px)다. 별도 폰트를 지정하지
않는다. 주 액션인 "다운로드" 버튼을 굵게(700) 처리해 1차 위계를 주고, 섹션 헤더
라벨("YouTube 링크", "최근 다운로드")도 볼드로 본문과 구분해 2차 위계를 둔다.
크기는 키우지 않는다 — 네이티브 데스크톱 관례상 굵기만으로 충분하다.

## Layout

간격은 세 단계 리듬을 따른다.

- **section (12px):** 섹션 사이, 좌우 거터의 기본 간격.
- **inner (6px):** 그룹 박스 내부 컨트롤 여백, 라벨-필드 가로 간격.
- **tight (4px):** 게이지와 상태줄처럼 밀접한 요소 사이.

창 최소 크기는 560×520px로 고정해 컨트롤이 겹치지 않게 한다. 입력창 높이는 90px,
다운로드 버튼 높이는 40px다.

## Components

- **url-input:** 흰 배경(surface-input)에 검정 텍스트. 멀티라인 링크 입력.
- **status-success / status-error / status-disabled:** 패널 배경(surface) 위에 각각
  성공(초록)·실패(빨강)·비활성(회색) 텍스트. 결과를 색으로도 구분한다.
- **button-download:** 패널 배경에 검정 굵은 글자, 높이 40px. 기본 버튼으로 지정되어
  포커스 시 accent 테두리가 표시되고 Enter로 실행된다.

## Localization

UI 문자열은 `core/i18n.py`의 `{lang: {key: text}}` 테이블에 모으고 `tr(lang, key)`로
조회한다. 한국어(ko)·영어(en)를 지원하며, 누락 키는 한국어 → 키 자체로 폴백해 번역
누락이 크래시가 되지 않게 한다. 언어 전환은 `wx.RadioButton` 등 위젯의 런타임 라벨
변경 제약 때문에 패널을 재생성(입력값·선택값 보존)해 다시 그린다.
