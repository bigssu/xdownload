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

## Colors

- **primary (#000000):** 기본 텍스트색(시스템 WINDOWTEXT). 라벨·입력·버튼 글자.
- **surface (#F0F0F0):** 패널 배경(시스템 BTNFACE). 라벨·상태줄·버튼이 올라가는 면.
- **surface-input (#FFFFFF):** 텍스트 입력창 배경(시스템 WINDOW).
- **disabled (#6D6D6D):** 비활성 텍스트(시스템 GRAYTEXT).
- **accent (#0078D7):** 기본 버튼(다운로드) 포커스 강조 테두리(시스템 HIGHLIGHT). OS가 그린다.
- **success (#008000):** 다운로드 완료 상태 메시지. `core` 코드의 `wx.Colour(0,128,0)`.
- **error (#B40000):** 실패/오류 상태 메시지. `wx.Colour(180,0,0)`.

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
