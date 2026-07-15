---
title: XChange UI 스타일·방식 이식 설계
date: 2026-07-15
status: approved
target_version: 0.68
---

# XChange UI 스타일·방식 이식 (XDownloader)

## 1. 목표

`D:\_NOAV\Xchange` 프로젝트의 **UI 스타일과 방식을 XDownloader에 그대로 이식**한다.
"스타일" = 딥 퍼플-틴트 다크 카드 팔레트 + Segoe UI 타이포 + 라운드 FlatButton/SectionCard.
"방식" = 하드코딩 금지 디자인 토큰(`c()/f()/dip()/PAD`) + 공유 위젯 모듈 + Sizer 전용 레이아웃
(출처: `Xchange/docs/UI_UX_RULES_XCHANGE_UI.md` 8개 규칙).

기능(다운로드·로컬 변환·히스토리·i18n)의 동작은 바꾸지 않는다. **순수 재스킨**이다.

## 2. 제약 (하드)

- **wxPython 4.2 고정** — 다른 프레임워크 금지(CLAUDE.md). Xchange도 wxPython이라 충돌 없음.
- **앱 용량 +10% 이내** — 기준선 `dist/XDownloader.exe` = 15.03 MB → **상한 16.54 MB**.
  신규 pip 의존성 0, 신규 대형 애셋 0 원칙 → 추가분은 순수 wx 코드(~360줄) → exe 증가 수십 KB 예상.
  **빌드 후 실측으로 최종 확인**(수용 기준: ≤ 16.54 MB).

## 3. 결정된 요구사항 (사용자 승인 2026-07-15)

- **테마 모델:** 라이트/다크 토글 **유지**, **둘 다 Xchange 파생** (다크=Xchange 원본, 라이트=파생 변형).
- **충실도:** **전체 채택** — 토큰 시스템 + 커스텀 위젯 전면 도입.

## 4. 접근 (채택: A)

- **A (채택):** `ui/theme_tokens.py` + `ui/widgets.py` 2개 모듈을 이식하되 **테마 인식(dark/light)으로 확장**,
  `main_frame.py`를 이 토큰·위젯으로 재스킨. 로직 계층은 무손상.
- B (기각): 모듈 없이 main_frame에 토큰 인라인 → Xchange의 "방식"(공유 토큰/위젯 모듈 규율) 훼손.
- C (기각): Xchange의 탭/툴바/ContentArea 구조까지 미러 → XDownloader엔 탭이 없어 매핑 불가, 오버킬.

## 5. 아키텍처

### 5.1 신규 모듈 `ui/theme_tokens.py`

Xchange 원본을 이식하되 **테마 인식**으로 확장한다.

- `FONT_BASE = 10`, `PAD_XS = 4`, `PAD_S = 8`, `PAD_M = 14` — 원본 그대로.
- `C_DARK` = Xchange `C` 딕셔너리 **원본 그대로**.
- `C_LIGHT` = 동일 키 집합, 라이트 파생 값(§5.2).
- 모듈 전역 `_theme = "dark"`, `set_theme(name)` 로 전환.
- `c(key)` — 현재 테마의 딕셔너리에서 조회, 캐시 키 `(theme, key)`.
- `f(delta, bold)` / `dip(px)` — 원본 그대로(테마 무관, 기존 캐시 유지).
- `set_theme()` 호출 시 색 캐시만 비운다(폰트/dip 캐시는 테마 무관이라 유지).

### 5.2 토큰 표 (dark = Xchange 원본, light = 파생)

| key | dark | light | 용도 |
|-----|------|-------|------|
| bg | #13121F | #F4F3FA | 프레임/패널 배경 |
| toolbar | #1A1928 | #ECEAF5 | 상단 영역 |
| card | #21203A | #FFFFFF | SectionCard 배경 |
| card2 | #2A2945 | #F2F1FA | 보조 카드 |
| surface | #2F2E50 | #EAE8F5 | 입력/드롭다운 배경 |
| status_bar | #100F1C | #E8E6F2 | 상태 영역 |
| border | #3B3A60 | #C9C6E0 | 버튼 테두리 |
| border_card | #302F52 | #DAD7EC | 카드 테두리 |
| btn_active_a | #5A3FCC | #5A3FCC | (공용) 활성 그라디언트 |
| btn_active_b | #3A72D4 | #3A72D4 | (공용) 활성 그라디언트 |
| btn_active_bd | #44EE88 | #44EE88 | (공용) 활성 테두리 |
| btn_normal | #252440 | #E4E2F2 | 일반 버튼 배경 |
| btn_hover | #302F52 | #D8D5EC | 호버 배경 |
| accent | #0078D4 | #0078D4 | (공용) 주 액션 |
| accent_h | #1A8AE0 | #1A8AE0 | (공용) 주 액션 호버 |
| secondary | #383762 | #E0DEF0 | 보조 버튼 배경 |
| secondary_h | #454470 | #D4D1E8 | 보조 버튼 호버 |
| toggle_on | #0078D4 | #0078D4 | (공용) 토글 ON |
| toggle_off | #3B3A60 | #C4C1DC | 토글 OFF |
| toggle_knob | #FFFFFF | #FFFFFF | (공용) 토글 손잡이 |
| text | #E6E4FF | #1E1D2E | 기본 텍스트 |
| text_dim | #7F7DAA | #6D6B8C | 흐린 텍스트 |
| text_card | #C8C5F0 | #3A3856 | 카드 본문 텍스트 |
| info | #5AB4FF | #0A6FC0 | 정보 |
| warn | #FFB930 | #B57400 | 경고 |
| gpu_dot | #3DDF84 | #1F9E57 | (미사용 가능) |
| pause_dot | #5A5880 | #9C99BE | (미사용 가능) |
| divider | #252440 | #DDDBEC | 구분선 |
| arrow | #7B6EE8 | #6A5DD6 | (미사용 가능) |
| danger | #B42D3A | #B42D3A | (공용) 위험 |
| danger_h | #DC3C4B | #DC3C4B | (공용) 위험 호버 |
| icon_disabled | #A8A8A8 | #9A98AE | 비활성 아이콘 |
| icon_disabled_acc | #848484 | #B8B6C8 | 비활성 강조 |
| icon_inactive | #E6E4FF | #3A3856 | 비활성 아이콘 |
| icon_active_acc | #44EE88 | #1F9E57 | 활성 강조 |
| icon_ready_acc | #5AB4FF | #0A6FC0 | 준비 강조 |
| copy_paper_bg | #21203A | #FFFFFF | (미사용 가능) |
| overlay_disabled | #484848 | #C0C0C0 | (미사용 가능) |

**XDownloader 전용 확장 토큰**(Xchange 상태줄/2테마 지원 위해 추가):

| key | dark | light | 용도 |
|-----|------|-------|------|
| ok | #4EC976 | #17803F | 완료 상태 메시지 |
| err | #F06969 | #B42D3A | 실패 상태 메시지 |
| on_fill | #FFFFFF | #FFFFFF | 채움형 버튼(accent/danger) 라벨색 |

> Xchange 원본 키는 전부 유지(미사용 키 포함) — 상류(Xchange) 동기화 용이성 우선. `ok`/`err`/`on_fill`만 확장.

**대비 검증 (dataviz 방법론 — 계산, 눈대중 금지):** 전 텍스트-배경 쌍의 WCAG 2.1 대비비를 스크립트로 산출.
- 라이트 `ok`를 #1B8A4B(4.39, AA 미달) → **#17803F(5.00, 통과)** 로 조정.
- FlatButton accent/danger 라벨을 `c("text")` → **`on_fill`(흰색)** 으로 변경 — 라이트에서 어두운 text가 파란 accent 위 대비 실패하던 결함 교정(§5.3). 다크는 기존과 사실상 동일.
- 다크 `text_dim`(원본 #7F7DAA, card 위 4.07)은 **Xchange 원본 그대로 유지** — dim/보조 텍스트이자 원본의 선택이므로 "그대로" 원칙 우선. bg 위에선 4.78로 통과.

### 5.3 신규 모듈 `ui/widgets.py`

Xchange 원본 위젯을 이식: `BasePanel`, `FlatButton`, `ToggleSwitch`, `DarkChoice`(OwnerDrawnComboBox),
`SectionCard`, `Divider`. `c()`가 테마 인식이라 위젯은 자동 재스킨된다.

- import 경로만 `from ui.theme_tokens import ...` 로 조정(Xchange의 `src.ui.theme_tokens` → `ui.theme_tokens`).
- **FlatButton 적응(2테마 대비):** accent/danger 스타일 라벨색을 `c("text")` → `c("on_fill")`(흰색)로 변경.
  라이트 테마에서 어두운 `text`가 채움 배경 위 대비 실패하는 것을 막는다. secondary/disabled는 기존대로 `c("text")`/`c("text_dim")`.
- 그 외 동작 로직은 원본과 동일.

## 6. main_frame.py 재스킨

로직(스레드 워커, 취소, 진행률, 히스토리, i18n)은 유지하고 **`_build_ui`/`_apply_theme`/`_palette`만 교체**한다.

| 기존 | 이식 후 |
|------|---------|
| `wx.StaticBoxSizer` × 4 (포맷/화질/변환/히스토리) | `SectionCard` |
| `wx.Button` (다운로드) | `FlatButton(btn_style="accent")` |
| `wx.Button` (폴더 변경, 변환 4개) | `FlatButton(btn_style="secondary")` |
| `wx.Choice` (화질) | `DarkChoice` |
| `wx.RadioButton` mp4/mp3 | `FlatButton` 2-버튼 세그먼트(active=accent) — **승인됨** |
| 커스텀 `_ProgressBar` | 유지 + 색 토큰화(`c("accent")` 채움 / `c("surface")` 트랙) |
| `한\|En` 토글, `☾/☀` 토글 | 유지 + 토큰 색 재스킨 |
| `_palette()` (dark 고정 + OS light) | 삭제 — `theme_tokens`가 대체 |

- 간격: `PAD_M`(12→14) 거터, `PAD_S`(8) 내부, `PAD_XS`(4) 밀접 요소. 모든 px는 `dip()` 통과.
- 레이아웃: Sizer 전용(규칙 B). 절대 위치 금지.
- 깜빡임: Frame/패널 `SetDoubleBuffered(True)`, FlatButton 비트맵 캐시(규칙 E) — 원본 위젯이 이미 내장.

## 7. 테마·언어 토글 메커니즘

- **테마 토글(☾/☀):** `theme_tokens.set_theme("light"|"dark")` → `config["theme"]` 저장 → **패널 재생성**
  (기존 `_rebuild_ui` 패턴 재사용: 입력·선택값 보존, Freeze/Thaw). FlatButton 비트맵 캐시 무효화 문제를
  재생성으로 원천 회피. *(ponytail: 캐시 무효화 로직 대신 재생성이 게으른 정답)*
- **언어 토글(한/En):** 기존대로 패널 재생성(변경 없음).
- 최초 실행 테마 초기값: 기존 로직 유지(OS 다크모드 감지).

## 8. 용량 검증 계획

1. 구현 후 `pyinstaller XDownloader.spec` 로 재빌드.
2. `dist/XDownloader.exe` 크기 측정.
3. **수용 기준: ≤ 16.54 MB** (기준선 15.03 MB의 +10%). 초과 시 원인(누락 애셋/의존성) 제거 후 재검.

## 9. 범위 밖 (non-goals)

- 다운로드/변환/히스토리/바이너리/i18n **로직 변경 없음**.
- Xchange의 탭/툴바/오버레이/ContentArea 구조 도입 안 함(매핑 불가).
- 신규 pip 의존성·폰트·이미지 애셋 추가 안 함(용량 제약).
- 기능 추가 없음(순수 재스킨).

## 10. 검증 (테스트)

- `python -m unittest`(converter) 회귀 통과 유지(UI 무관하지만 리그레션 가드).
- `py_compile` 전 UI 파일.
- **실기동 스모크**: 앱 실행 → 다크/라이트 토글 → 한/En 토글 → 다운로드 버튼/변환 버튼/화질 드롭다운/진행바
  렌더링 육안 확인(위젯 ghost·깜빡임 없음, 규칙 C/E 준수).
- 빌드 후 exe 용량 실측(§8).

## 11. 컨벤션 결정 (확정 — 사용자 승인 2026-07-15)

- **들여쓰기:** 신규 2개 모듈은 XDownloader 관례(**탭**)로 이식. Xchange 원본은 4-space지만, 프로젝트 일관성
  (CLAUDE.md "기존 스타일 준수")을 위해 탭으로 재포맷. 로직은 불변.
- **타입 힌트/docstring:** CLAUDE.md "불필요한 annotation 금지"에 맞춰 이식 시 과한 힌트는 정리(로직 불변).

## 12. DESIGN.md 갱신

기존 "OS 시스템 색을 따른다" 원칙을 **Xchange 파생 토큰 시스템**으로 대체하도록 `DESIGN.md`를 갱신한다
(팔레트·타이포·간격 토큰·커스텀 위젯 규칙 반영).

## 13. 리스크

- **테마 전환 재생성 비용:** 위젯 수가 적어 무시할 수준. Freeze/Thaw로 깜빡임 억제.
- **DarkChoice/OwnerDrawnComboBox 다크 팝업:** MSW 네이티브 한계 잔존 가능 → 원본 위젯이 이미 대응.
- **용량:** 순수 코드라 상한 여유 크나, PyInstaller가 새 모듈로 인해 예상외 수집을 하지 않는지 실측 확인.
