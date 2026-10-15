# v0.4 검증 기록 — 2026-10-14

서기 보고서, 인용·BibTeX·HTML 내보내기, 토론 이어 가기, 이어서 묻기를 추가한 판본의 검증이다. 내용과 이유는 [report-v0.4.md](report-v0.4.md)를 본다.

## 자동 검증

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` 통과. `cargo test --workspace`: **73개 통과** (core 15, engine 단위 19, 토론 13, 공유 폴더 7, 실행·저장 14, GPUI 5). 세부 목록은 report-v0.4.md의 ‘검증’ 절에 있다. GPUI 테스트는 실제 구독 호출을 보내는 버튼을 누르지 않는다.

## 실제 창과 내보낸 파일

Xvfb + Mesa(Noto CJK 글꼴)에서 `demo_project` 데이터(스크립트 응답)로 보고서 탭 전체(답, 핵심 내용, 개념도, 표, 용어 풀이, 다음 단계, 참고문헌, 이어서 묻기 스레드, 토론 이어 가기), 이론 상세, 토론 탭 끝의 서기 단계와 이어 가기, 내보내기 대화상자를 xdotool로 클릭해 확인했다. 내보낸 `report.html`은 Chromium(Playwright)으로 1280px 라이트, 390px 다크에서 확인했고 개념도 상자를 눌러 설명이 열리는 것을 확인했다.

## 미검증

- 실제 Claude·Codex 구독으로 서기 보고서·이어 가기·후속 질문을 실행하지 않았다(이 환경에 로그인 없음). 보고서 품질은 실제 질문으로 확인해야 한다.
- PubMed efetch·Crossref·doi.org 서지 보완의 실제 네트워크 호출(파서, 제목 일치 판정, 주소의 DOI 추출, 레코드 병합만 검증).
- macOS 빌드·번들.

---

# v0.3 검증 기록 — 2026-10-13

이 절은 Linux 클라우드 환경에서 수행한 v0.3 검증이다. 아래 v0.2·v0.1 기록은 당시 기록으로 보존한다. 변경 내용과 진단은 [diagnosis-v0.3.md](diagnosis-v0.3.md)를 본다.

## 자동 검증

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` 통과. `cargo test --workspace`: **54개 통과** (core 8, engine 단위 15, 토론 7, 공유 폴더 7, 실행·저장 14, GPUI 3).

- 토론(`crates/engine/tests/deliberation.rs`, fixture provider): GPT 구성 · Claude 비판 교대 순서, 1회 수정 후 수렴, 비판 이력(응답 → 판정), 저자가 넣은 판정 무시, 원장에는 수정된 마지막 판본만, 모델이 namespace 밖에 만든 ID 재부여와 인용 참조 갱신, 같은 이견 2회 유지 → "진전 없음" 종료와 원장 '이견 있음', 빠르게 범위(수정 0회), 서버 혼잡 1회 재시도, 비판 모델 로그인 실패 시 대체 호출 없음 + 부분 답변, 재시도로 호출이 모자랄 때 단계 보류 후 종합, Markdown 내보내기의 토론 기록·비판 원장.
- core: 항목 단위 격리(가짜 인용·전송 범위 밖 인용 제외, 없는 의존·수정 대상 해제, 순환 제거, 심각도 정규화), ID namespace 재부여, 수정 판본 연결의 head 계산, strict 스키마(새 필드 포함 모든 속성 required).
- 실행·저장: 기존 회귀 + 출처 40개를 요청 한도 안에 배분.
- GPUI(실제 컴포넌트): 프로젝트 대화상자에서 한국어 제목 입력·생성, 예시 질문, 한국어 입력과 일반 Enter 비실행, 검토 패널, 자동 저장 전 프로젝트 전환 시 초안 보존, 실제 엔진이 만든 토론 실행의 모든 단계 렌더와 비판 경과 열기.
  - 원래 테스트는 대화상자 열림 애니메이션 도중 클릭해 입력이 들어가지 않았다(이 환경에서 원본 코드로 재현). 테스트 시계를 진행시켜 애니메이션이 끝난 뒤 조작하도록 고쳤다.

## 실제 창

Xvfb + Mesa에서 실제 GPUI 창을 띄우고 xdotool로 클릭해 첫 화면, 프로젝트 대화상자, 답변·토론·근거·작업 기록 탭, 주장·가정, 자료, 설정, 원문·비판 검토 패널, 다크 모드를 확인했다. 데모 데이터는 `demo_project` 예제의 스크립트 응답(가상)이다. 설정 화면은 이 환경의 실제 상태(Claude Code CLI 설치·미로그인, Codex CLI 없음)를 표시했다.

## 미검증

- 실제 Claude·Codex 구독 호출. 이 환경에 로그인이 없다. Claude Code CLI 2.1.296의 플래그(`--safe-mode`, `--json-schema`, `--tools`, `--setting-sources`, `--no-session-persistence`, `--disable-slash-commands`)가 존재함은 `claude --help`로 확인했다.
- 실제 모델이 새 `critique_responses`/`critique_reviews` 계약을 지키는 정도와 토론의 연구 품질.
- macOS 빌드·번들·서명과 PDFKit 경로(Linux에서 작업). Linux `pdftotext` 경로는 컴파일만 확인했다.

---

# v0.2 검증 기록 — 2026-10-13

이 절은 아래 v0.1 기록을 갱신한다. 과거 실패와 미검증 상태는 당시 기록으로 보존했다.

## 자동 검증

`cargo test --workspace`: **41개 통과** (core 5, engine/provider/search 15, 공유 폴더 7, 실행·저장 13, native GPUI 1). `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, release bundle 및 ad-hoc 서명 검사를 통과했다. upstream `block 0.1.6`의 향후 Rust 호환성 경고는 남아 있다.

인용은 단어·문장부호가 원문과 일치하고 전송 구간에서 유일한 경우에만 줄바꿈·행 번호를 결정적으로 보정한다. 반복 구절, 단어 중간을 자른 구절, 원문 밖 인용, 바뀐 단어는 자동 보정하지 않는다. 모델 원본 응답과 검증된 출력, 보정 기록을 요청·응답 패널에 따로 남긴다.

회귀 검증에는 optional/nullable 필드까지 포함한 Codex strict schema, critique severity enum, 전이적 선행 작업 문맥, 실제 progress 전달, 질문 기반의 일관된 원문 구간/원래 행 번호, 조작 인용 차단, 일부 작업 실패 상태, 질문 초안의 즉시 프로젝트 전환 보존, 자료 변경·삭제·이전 내용 복원·재가져오기·취소 롤백·writer lock·재시작·EPUB spine 순서·DOCX 추출·확장자가 붙은 기존 자산의 hash 검증 복구를 포함한다.

## 실제 자료와 구독

- 실제 PDF `Brain-Dynamics-for-Psychiatrists.pdf`: 60/60페이지, 1,736행 추출. 실제 EPUB `Mechanism of the Action of Psychedelics.epub`: 4개 섹션, 577행 추출. 격리 폴더에서 재열기와 중복 방지를 확인했다. 본문 전체가 모든 모델 요청에 전달된다는 의미는 아니다.
- Codex `gpt-6.1-sol`과 Claude `claude-opus-5-5` 각각 멜라토닌 질문의 planner → 후보 2개 → 비판 → 종합, **5회 호출 모두 성공**. 이 두 실행은 문헌 검색 연결 전의 자료 없는 경로이며 `evidence_insufficient`로 정확히 종료했다. `.runtime/codex-full-check-v2`, `.runtime/claude-full-check-v2`에 결과와 bundle을 보존했다.
- 실제 PubMed + ScholarPen TinyFish 검색: `melatonin anxiety receptor mechanism`에서 8건 확보(초록 4, 웹 발췌 2, 서지정보만 2). 검색 실패 경고 없음. 제목/검색 snippet을 본문 근거로 사용하지 않는다.
- 설치본 설정에서 Codex와 Claude 모두 초록색 **연결됨 · 구독 인증 확인**을 직접 확인했다. Claude의 남은 구독량은 여전히 알 수 없으며 인증 성공과 구분한다.

## 설치본 저장 및 실패 경로

기존 멜라토닌 프로젝트를 ScholarPen 프로젝트 폴더로 공유했다. 프로젝트 ID, 이전 실패 기록, 질문, 첨부 원본을 보존했다. `.theory-studio/project.sqlite`에 저장하며 로컬 원본 프로젝트는 백업으로 남긴다. Google Drive 루트의 느린 디렉터리 조회로 앱 시작이 멎는 실제 현상을 발견해 자동 발견을 백그라운드로 옮겼다.

설치본에서 같은 질문을 검색 사용 상태로 실행하여 문헌 8건과 검증된 근거 인용 8개, 두 기전 분석을 저장했다. 이어진 Codex 비판 작업은 `serverOverloaded / Selected model is at capacity`로 중단됐다. 이는 앱 schema 실패와 구분되며, 중간 분석과 요청·응답은 저장했다. 모델 혼잡 안내를 한국어로 표시하도록 개선했다. 기존 자산의 확장자와 저장 경로 불일치도 재시작 점검에서 발견했고 확장자가 붙은 원본과 이전 확장자 없는 원본을 모두 읽고, 내용을 hash로 검증하도록 수정했다. 실제 사용자 첨부 PDF의 32/32페이지, 3,564행을 추출해 저장했다. 후속 설치본 검증에서 모델이 PDF 줄바꿈을 합치면서 끝 행을 잘못 반환한 사례를 발견했고 위의 제한적인 인용 위치 보정을 추가했다. 과거 실패 실행은 그대로 보존한다.

## 수정 후 실제 설치본 전체 실행

`gpt-6-sol`을 UI에서 명시적으로 선택해 같은 멜라토닌 질문을 다시 실행했다. `run-c8c75ff0f8b54c82bc0c634e51400b9d`는 **276초, 5회 호출 모두 성공, answered**로 종료됐다. planner → direct_constructor → mediated_constructor → mechanism_critic → synthesis 전체 경로가 통과했고 원문 대조 인용 **20개**를 저장했다. 새 검색 자료 9건이 추가되어 프로젝트 자료는 27건(본문/초록/발췌 23건, 메타데이터만 4건)이 됐다. 기존 검증 자료가 누적된 수치이며 27개 독립 논문 전체를 읽었다는 뜻은 아니다.

첫 분석의 PDF 인용 1개는 단어·문장부호가 정확히 같은 원문을 찾아 행 153–155로 보정했다. 요청·응답 패널에서 보정 기록이 보이는 것을 직접 확인했다. UI는 ‘답변 작성됨’ 상태와 결과, 가설의 제한을 표시했다. 이전 schema·serverOverloaded·인용 위치 실패 기록은 보존했다. 이 실행 성공은 제안된 신경생물학 기전이 참이라는 전문가 인증이 아니다.

Google Drive의 공유 폴더에서 확장자 없는 PDF 자산에 `.pdf`가 붙는 현상도 실제 export 검사에서 재현됐다. 새 자산과 bundle v2는 처음부터 `hash.pdf`처럼 형식에 맞는 확장자를 사용한다. 이전 확장자 없는 bundle v1도 읽으며, 가져오는 원본 bundle을 수정하지 않는 회귀 검증을 추가했다.

최종 설치본을 다시 실행해 판본 7, 4번째 질문의 `answered` 결과, 한국어 초안과 연결된 가설·전제 버튼을 확인했다. 직접 경로 버튼이 해당 주장과 의존 가정·원문 인용을 검토 패널에 여는 것도 확인했다. 검증용 `gpt-6-sol` 선택은 원래 `gpt-6.1-sol`로 복원했다.

설치본 native 폴더 창에서 ScholarPen 프로젝트의 `exports/theory-1791900868-48681c97.theorybundle`을 저장했다. bundle v2의 manifest/project hash, PDF 자산 hash, 최신 실행의 인용 20개(출처 hash·원문 행), SQLite `integrity_check=ok`를 직접 검증했다. 최종 빌드/설치 바이너리 SHA-256은 `fcc106a452dcacc4a9adaaece0d07851e2ad0aab94b20620acf745f1361642f3`로 일치하고 ad-hoc 서명 검사도 통과했다.

## 범위

PDF/EPUB/DOCX 본문 추출과 자동 문헌 검색은 구현됐다. 스캔 PDF OCR, 유료 논문 원문 접근, 수치 시뮬레이션, 자동 재계획/수렴 반복, 분기 병합, 모든 연구 유형의 전문가 검토는 완료 범위가 아니다. 검색은 실행당 최대 2개 검색어이며, 확보한 문헌과 전달한 구간을 근거로 분석한다. 서버 혼잡이나 구독 한도를 우회하지 않는다.

---

# v0.1 초기 검증 기록 — 2026-10-13

## 검증 환경

Apple Silicon macOS, Rust 1.99.0, gpui-kit 0.7.1, Cargo.lock 사용. Codex CLI 0.160.0과 Claude Code 2.1.296을 탐지했다. release bundle은 ad-hoc 서명이며 설치 경로는 `~/Applications/Theory Studio.app`이다. 터미널의 개발용 PATH를 제거한 `env -i HOME=… PATH=/usr/bin:/bin open …`으로 설치본을 열었다. 실행 중인 프로세스의 경로와 빌드/설치본 SHA-256 일치를 확인했다.

## 자동 검증

- `cargo fmt --all --check` 통과.
- `cargo clippy --workspace --all-targets -- -D warnings` 통과.
- `cargo test --workspace`: 25개 통과 (core 4, provider 9, 저장·실행 통합 11, 실제 GPUI 컴포넌트 1).
- `cargo build --release --locked -p theory-studio`, `scripts/bundle.sh release`, `codesign --verify --deep --strict` 통과.
- 전이 의존성 `block 0.1.6`의 향후 Rust 호환성 경고가 있다. 현재 고정 버전의 빌드·검사 실패는 아니다.

자동 검증은 잘못된 계획, 독립 후보 문맥 격리, 가짜 인용, 원문 위치, 관련 의존성의 전이적 선택, 수정 대상, 늦은 취소 응답, 재시작 복구, 인증 실패, API 인증 거부, Codex 포함 사용량 차단, 조기 이벤트와 중복 완료, 잘린/과대 응답, 프로세스 취소, stdout 종료 후 hang, bundle hash·경로 조작을 포함한다. GPUI 테스트는 실제 컴포넌트의 프로젝트 생성, 한국어 입력, 일반 Enter와 실행 구분, Inspector 열기·닫기를 확인한다.

## 실제 구독 연결과 연구 사례

Claude Code 구독 인증으로 `claude-opus-5-5`의 구조화 응답을 받았다. API 키를 사용하지 않았다. 최초 실행에서 설치된 Claude JSON validator가 JSON Schema 2020-12 URL을 거부했다. 공통 스키마 부분집합을 유지하고 해당 meta-schema URL을 제거해 해결했다. 최초 실패 이력은 프로젝트에 남아 있다.

앱 화면에서 한국어 프로젝트를 만들고 아래 두 질문을 각각 Quick 4회 예산으로 실행했다. Planner와 세 작업의 실제 응답 및 사용량은 SQLite Attempt에 저장됐다.

1. **원문 범위 대조**: [Beggs & Plenz (2003)](https://pmc.ncbi.nlm.nih.gov/articles/PMC6741045/)의 표본 준비를 설명하는 짧은 Methods 구절만 제공했다. 이 구절로 살아 있는 사람의 뇌 전체에 대한 avalanche 관찰을 주장할 수 있는지 물었다. `EvidenceReviewer → Critic → Synthesizer` 계획을 만들었고 4회 모두 성공했다. 표본 불일치와 근거 부재를 구분하고 `evidence_insufficient`로 종료했다. 비평가는 더 조심스러운 수정안을 기존 주장과 분리해 제안했다. 답변의 근거 버튼을 눌러 실제 구절·행 1·제공 발췌 범위를 Inspector에서 확인했다.
2. **개념 구분**: criticality와 metastability의 정의와 구별 관찰을 요청하면서 위 자료는 사용하지 말라고 지시했다. `Analyst → Critic → Synthesizer`, 각 `source_ids=[]`인 다른 계획이 생성됐다. 4회 모두 성공했고 문헌을 확인하지 않은 설명을 이론적 제안으로 표시했다. `evidence_insufficient`로 종료했다. 이는 과학적 내용 전반에 대한 전문가 검토의 통과를 뜻하지 않는다.

Codex는 앱 전용 App Server의 초기화·계정 조회가 동작했고 미로그인 상태를 올바르게 표시했다. 사용자 로그인이 필요한 상태이므로 실제 Codex 생성, 브라우저 로그인 완료, 두 provider 교차 검토는 **미검증**이다. transport fixture 성공과 실제 인증 성공을 혼동하지 않는다.

## 설치본·저장 검증

- release 설치본을 다시 열어 두 질문, 원문, 19개 제안, 한국어 프로젝트명과 초안이 보존된 것을 확인했다.
- 원문의 표본 주장 하나를 UI에서 채택하고 철회했다. 해당 주장과 의존하는 5개 항목만 `needs_review`가 됐고 무관한 개념 질문 항목은 그대로였다. 판본 4의 원장을 새로운 판본 7로 복원해 19개 제안이 원래 상태로 돌아왔으며 실행·변경 이력은 남았다.
- 설치본의 native 폴더 선택 창으로 `.theorybundle`을 내보냈다. `project.json`, `report.md`, SQLite backup, manifest, assets 디렉터리를 실제로 읽었다. 판본 7과 두 결과, 최초 실패 이력이 남고 SQLite `integrity_check=ok`였다. bundle 재가져오기·자산 hash는 자동 통합 테스트에서 확인했다.
- 설치본에서 실제 Claude 호출을 시작한 뒤 **중단**을 눌렀다. 1회 예약 이력과 `cancelled` 상태가 저장됐고 원장 19개 항목은 바뀌지 않았다. 원격 서버의 계산 종료·구독 사용량 반환 여부는 로컬 취소만으로 확인할 수 없다.
- 중단 뒤 설치본을 종료하고 다시 열어 중단된 질문과 초안, 이전 성공 결과가 보존됐고 자동으로 호출을 재개하지 않는 것을 확인했다.

## 출시 범위와 남은 검증

실행 가능한 첫 로컬 버전이며 계획서의 모든 M0–M5 수락 조건을 완료했다는 의미는 아니다. PDF 본문 추출/OCR, 자동 검색, 수치 계산, 자동 의미 재계획과 수렴 회차, 비판 처분 전이, 분기 병합은 구현하지 않았다. 비판은 미해결 제안으로 보존하고 후속 질문에서 재검토한다.

여덟 연구 유형의 평가 계약은 `workflow-validation-cases.json`에 있다. 실제 모델 연구 검토는 위 두 사례만 수행했다. 여덟 사례 전체의 전문가 검토 및 단일 호출 대비 비교 평가는 남아 있다. 실제 한국어 텍스트 입력·보존은 검증했지만 macOS 한글 IME 조합 중 모든 단축키, 다크 모드·최소 창·확대의 전체 조합, 실제 강제 종료 순간별 fault injection, 공증·다른 Mac 배포는 별도로 검증해야 한다.
