# Theory Studio

신경생물학 이론 탐구를 위한 로컬 macOS 데스크톱 앱. Rust + GPUI Kit, SQLite, Codex App Server와 Claude Code 구독 연결을 사용합니다.

## 실행

설치본: `~/Applications/Theory Studio.app`. 첫 검증 프로젝트와 실제 Claude 실행 기록은 앱에서 열 수 있습니다. 자세한 검증 범위는 [QA 기록](docs/QA.md)을 참고하세요.

```sh
cargo run --locked -p theory-studio
```

Apple Silicon macOS에서 검증합니다. Rust 1.99.0은 `rust-toolchain.toml`로 고정합니다. UI 진입점은 `gpui-kit = 0.7.1`이며 전이 의존성의 실제 버전은 `Cargo.lock`에 고정되어 있습니다. 별도 웹 서버·Python worker·LLM API 키가 필요하지 않습니다. TinyFish 검색은 ScholarPen에 이미 저장된 API 연결을 재사용하며, 연결이 없으면 PubMed를 사용합니다.

```sh
scripts/bundle.sh release
open 'dist/Theory Studio.app'
```

로컬 bundle은 ad-hoc 서명입니다. Developer ID 서명·공증된 배포판은 아닙니다. Codex와 Claude CLI는 앱에 포함하지 않고 설치된 공식 CLI를 탐지합니다. Finder 환경에서도 `~/.local/bin`, `/opt/homebrew/bin`, `/usr/local/bin`을 확인하며 설정에서 경로를 지정할 수 있습니다.

## 사용 순서

1. **프로젝트 폴더 열기…**에서 ScholarPen 연구 폴더를 선택합니다. ScholarPen의 프로젝트 목록도 자동으로 나타납니다. 새 프로젝트는 ScholarPen의 프로젝트 루트에 만듭니다. 기존 앱 내부 프로젝트는 **연구 방향 → 이 연구를 ScholarPen 폴더로 저장**으로 기록을 옮깁니다. 원본은 백업으로 남습니다.
2. **설정**을 열면 Codex와 Claude의 인증 상태를 자동으로 조회합니다. 필요하면 **연결 확인**으로 새로 조회합니다. 인증 상태와 남은 사용량은 별도로 표시합니다. **응답 테스트**는 실제 구독 호출 1회를 사용합니다. Codex는 앱 전용 ChatGPT 로그인, Claude는 기존 Claude Code 구독을 사용합니다.
3. `resources` 또는 `resource` 아래의 논문·책을 자동으로 읽습니다. PDF, EPUB, DOCX, Markdown, TXT, QMD를 지원합니다. 원문 읽기 실패 사유는 **자료**에서 확인합니다. 스캔 PDF는 OCR이 필요합니다.
4. **연구 방향**에 프로젝트 목적·관점·제약을 자유롭게 적습니다. 특정 학문 분야의 고정 체크리스트는 없습니다.
5. 질문을 입력하고 **질문 실행**을 누릅니다. **문헌 검색**은 기본 켜짐입니다. 필요한 경우 계획자가 만든 검색어로 PubMed와 TinyFish를 조회하고 확보한 자료를 분석·비판·종합에 전달합니다. TinyFish는 ScholarPen에 저장된 연결을 엔진에서만 읽습니다. 로컬 자료만 사용하려면 문헌 검색을 끕니다.
6. 입력창 바로 위에서 현재 단계, 경과 시간, 작업 완료 수, 중단 버튼을 확인합니다. **진행 상황 보기**로 각 작업을 펼치고 **요청·응답 보기**로 기술 기록을 엽니다. 오류 때문에 일부만 끝난 결과는 **부분 답변**이며, 문헌의 한계인 **근거 부족**과 구분합니다.
7. 답변의 근거 버튼은 실제 확보한 구절과 행 번호를 엽니다. PubMed 초록, 웹 페이지 발췌, 제목만 확보한 결과를 구분합니다. 검색 결과가 있다는 사실만으로 주장이 입증되지는 않습니다.
8. **연구 노트**에서 후보·주장과 용어·가정을 검토하고 채택·철회·후속 질문을 합니다. **판본**에서 과거 원장을 새 작업안으로 복원하고, **내보내기…**에서 Markdown·JSON·SQLite·첨부 원문을 포함한 bundle을 저장합니다.

`⌘N` 새 질문 · `⌘Enter` 실행 · `⌘O` 자료 추가 · `⌘F` 자료 검색 · `⌘E` 내보내기 · `⌘,` 설정. 일반 Enter는 여러 줄 입력이며 실행하지 않습니다. Esc는 GPUI의 최상위 대화상자를 닫습니다.

## 데이터와 실행 경계

- 공유 프로젝트의 연구 기록: `<ScholarPen project>/.theory-studio/project.sqlite` 및 자산. ScholarPen 원고는 변경하지 않습니다. 연결 설정·인증은 `~/Library/Application Support/Theory Studio/`에 별도로 보관합니다.
- 이전 로컬 프로젝트는 `projects/<id>/project.sqlite`에 보존됩니다. 공유 자료의 변경·삭제는 이전 인용 판본을 지우지 않고 비활성화합니다. DB는 WAL + FULL 동기화, 변경은 transaction입니다. 원장은 SQLite가 기준이고 JSON/Markdown은 내보내기 형식입니다. 새 bundle은 v2로, 자산에 원래 형식의 확장자를 유지합니다. 기존 v1 bundle도 가져올 수 있습니다.
- 앱 수준 단일 writer 잠금. 두 인스턴스가 같은 폴더를 쓰면 두 번째 인스턴스는 오류를 표시합니다.
- 예산 예약·요청 context를 호출 전에 저장합니다. 작업 완료는 검증을 거쳐 저장합니다. 재시작 시 미완료 작업을 `interrupted`로 바꾸고 자동 재전송하지 않습니다.
- 모델 출력에는 구조, ID, 의존 관계, 출처 hash, 정확한 인용 문자열과 행 범위 검사가 적용됩니다. 원문과 단어·문장부호가 일치하는 유일한 구절의 줄바꿈·행 번호는 원문대로 보정하며, 원본 응답과 보정 기록을 따로 남깁니다. 이 검사는 논문의 결론이나 모델 추론이 참임을 보장하지 않습니다.
- 모델에 shell과 파일 쓰기를 허용하지 않습니다. 문헌 검색은 앱의 읽기 전용 PubMed/TinyFish 연결에서 수행합니다. 실행당 검색어 최대 2개, 검색어당 PubMed 최대 5건·TinyFish 최대 4건을 조회합니다. 출처마다 질문과 어휘가 겹치는 구간에서 12 KB 이내의 완전한 행을 전달하며 범위와 생략량을 기록합니다. 전체 요청이 240 KB를 넘으면 중단합니다. 책 전체를 검토했다고 표시하지 않습니다.
- 초안과 모델 역할은 별도 문맥입니다. 독립 구성 작업은 서로의 산출물을 받지 않습니다. 교차 검토를 켜면 Critic에 다른 provider를 사용하며 연결 실패 시 조용히 동일 provider로 바꾸지 않습니다.
- 호출은 순차 실행합니다. 취소는 새 호출을 막고 Codex interrupt 및 소유 프로세스 그룹 정리를 수행합니다. 원격 완료·과금 여부는 취소 뒤 확인 불가일 수 있습니다.
- API 결제 경로·자동 fallback이 없습니다. 구독 사용량은 계정의 다른 앱과 공유합니다. Codex의 포함 사용 허용 여부가 불명확하면 중단합니다. Claude는 구독 인증을 확인하며 계정의 추가 사용 정책은 Claude에서 관리합니다.

## 현재 범위

v0.2는 실제 실행 오류와 사용자 작업 흐름을 수정한 로컬 버전입니다. 계획서 전체의 완료를 뜻하지는 않습니다. 동적 계획, 기본 연구 역할, 원문 원장, 판본과 후속 질문, 두 구독 provider, 저장·취소·복구·portable export를 제공합니다.

다음은 별도 후속 구현입니다: 스캔 PDF OCR, 유료·접근 제한 원문 다운로드, 수치 실행, 시각적 주장 그래프, 분기 병합, 의미 기반 자동 재계획·다회차 수렴, 검토자에 의한 비판 처분 변경. 현재 비판은 미해결 제안으로 보존하며, 사용자가 후속 질문으로 다시 검토합니다. 스키마 오류인 계획은 한 번 수정 요청한 뒤 예산이 허용하면 보수적 recipe를 사용합니다. 원격 호출은 자동 재시도하지 않습니다.

## 검증

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p theory-engine --example provider_check -- claude
cargo run -p theory-engine --example provider_check -- codex
# 아래 명령은 실제 구독 호출 1회를 사용합니다.
cargo run -p theory-engine --example provider_check -- claude --generate
```

Fixture provider는 테스트에만 있습니다. UI에는 데모 응답을 실제 모델 응답으로 표시하는 경로가 없습니다. `docs/QA.md`에 실제 실행 근거와 미검증 항목을 기록합니다. 여덟 연구 사례의 수동 평가 계약은 `docs/workflow-validation-cases.json`에 있습니다.
