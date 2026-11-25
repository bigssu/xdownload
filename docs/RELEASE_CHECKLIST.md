# 배포 준비 점검표 — Xdownload2 2.0.0 (포털에서 exe 직접 받기)

마지막 확인: 2026-11-25. 항목마다 근거를 적었다. **준비율 20 / 21 = 95%.**
남은 1개(코드 서명)는 인증서가 있어야 해서 이 저장소 안에서는 끝낼 수 없다.

## 제품

| # | 항목 | 상태 | 근거 |
|---|---|---|---|
| 1 | 기능이 계획대로 동작 | ✅ | `docs/plans/V2_PLAN_20261125.md` 범위 전부 구현 |
| 2 | 단위 테스트 통과, 빌드 경고 0 | ✅ | `cargo test` 58 통과(debug), release 57 통과 + 수동 4 |
| 3 | 실제 네트워크 검증 | ✅ | `cargo test --release live -- --ignored`: 동시 다운로드, MP4/MP3/M4A, 자막·썸네일 포함 확인, 실패 링크 격리, GPU·CPU 변환, GIF/WebP/JPG |
| 4 | 실제 창 조작 검증 | ✅ | `packaging/ui_smoke.ps1`: 탭 전환 → 다운로드 버튼 → 파일·기록 생성 → 목록 지우기 → 닫기 → 설정 유지 |
| 5 | 화면 확인 (한국어·영어) | ✅ | `--screenshot` 로 네 탭 확인 |
| 6 | 성능 실측 | ✅ | 다운로드 3개 26.2→14.7초, 720p 변환 183→54초(NVENC), 이미지 24장 5.9→1.0초 |
| 7 | 취소 시 프로세스가 남지 않음 | ✅ | `proc` 테스트(손자 프로세스 포함), 창 닫기 = 전체 취소 |
| 8 | 오프라인·실패 처리 | ✅ | 준비 실패는 아래 줄에 표시 후 필요할 때 재시도, 항목 실패는 그 항목만 |
| 9 | 충돌 기록 | ✅ | `%APPDATA%\Xdownload2\logs\crash.log` (`crashlog` 테스트) |
| 10 | 접근성 | ✅ | 모든 버튼·선택·스위치에 이름(UI 자동화 트리 확인), 글자 대비 WCAG AA 테스트 |

## 배포물

| # | 항목 | 상태 | 근거 |
|---|---|---|---|
| 11 | 한 번에 만드는 배포 스크립트 | ✅ | `pwsh packaging\release.ps1` → `dist\` (테스트·빌드 실패 시 중단, 낡은 exe 를 먼저 지움) |
| 12 | exe 제품 정보·버전·아이콘 | ✅ | 속성 > 자세히: Xdownload2 / 2.0.0 / 저작권 (스크립트가 버전 일치 검사) |
| 13 | 다른 PC 에 추가 설치 없이 실행 | ✅ | C 런타임 정적 링크: exe 가 vcruntime/msvcp DLL 을 참조하지 않음 |
| 14 | 무결성 값 | ✅ | `dist\xdownload2.exe.sha256`, `xdownload2.json` 의 sha256 |
| 15 | 백신 검사 | ✅ | Windows Defender 사용자 지정 검사: 위협 없음 |
| 16 | 새 버전 알림 | ✅ | 앱이 `…/dl/xdownload2.json` 을 읽어 더 높으면 제목줄에 알림(신뢰 주소만, 테스트 있음) |
| 17 | 코드 서명 | ❌ | 인증서 없음. 받는 사람에게 SmartScreen 경고가 뜬다. 인증서가 생기면 `release.ps1 -CertThumbprint <지문>` |

## 문서

| # | 항목 | 상태 | 근거 |
|---|---|---|---|
| 18 | 사용 안내 | ✅ | `docs/USER_GUIDE_KO.md` (SmartScreen 넘기는 법, 문제 해결, 데이터 위치, 지우는 법) |
| 19 | 변경 내역 | ✅ | `CHANGELOG.md` (스크립트가 현재 버전 항목을 요구) |
| 20 | 오픈소스 고지 | ✅ | `THIRD_PARTY_NOTICES.md` (`python packaging/notices.py` 로 재생성) |
| 21 | 개발 문서 | ✅ | `README.md`, `DESIGN.md`, `docs/plans/` |

## 출시 순서

0. 실행 중인 xdownload2.exe 를 닫는다(`dist\` 의 exe 를 쓰는 중이면 스크립트가 멈춘다).
1. `Cargo.toml` 과 `assets/xdownload2.manifest` 의 버전을 올리고 `CHANGELOG.md` 에 항목을 쓴다.
2. `pwsh packaging\release.ps1`
3. `pwsh packaging\ui_smoke.ps1` (창이 잠깐 뜬다)
4. `dist\` 의 세 파일(`xdownload2.exe`, `xdownload2.exe.sha256`, `xdownload2.json`)을 포털 `/dl/` 에 올린다.
   **exe 를 먼저, json 을 마지막에** 올린다(json 이 올라가는 순간 기존 사용자에게 알림이 뜬다).
5. 포털 카드의 받기 주소를 `/dl/xdownload2.exe` 로 바꾸고 `docs/USER_GUIDE_KO.md` 내용을 안내로 붙인다.

## 점검표 밖에서 직접 해야 하는 것

- **포털 업로드**: 서버 접근 권한이 필요하다.
- **다른 PC 확인**: 개발 도구가 없는 Windows 10/11 에서 한 번 실행해 본다(정적 링크는 확인했지만 실제 깨끗한 PC 실행은 해 보지 못했다).
- **앱 라이선스**: 저장소에 `LICENSE` 가 없다. 소스를 공개 배포하려면 정한다.
- **HDR·다중 모니터·150% 외 배율**: 이 PC(150%) 에서만 봤다.
