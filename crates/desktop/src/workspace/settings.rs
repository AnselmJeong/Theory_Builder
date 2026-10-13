use super::*;
use gpui_kit::component::{checkbox::Checkbox, radio::RadioGroup};
impl Workspace {
    pub(super) fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Settings;
        if !self.busy() {
            self.service.cancel.store(false, Ordering::SeqCst);
            for provider in [ProviderKind::Codex, ProviderKind::Claude] {
                if !self
                    .connections
                    .iter()
                    .any(|c| c.provider == provider.label())
                {
                    self.submit(Command::Inspect(provider), cx);
                }
            }
        }
        cx.notify();
    }
    pub(super) fn sync_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for (field, value) in [
            (&self.codex_model, &self.settings.codex_model),
            (&self.claude_model, &self.settings.claude_model),
            (&self.codex_path, &self.settings.codex_path),
            (&self.claude_path, &self.settings.claude_path),
        ] {
            field.update(cx, |s, cx| s.set_value(value.clone(), window, cx));
        }
    }
    pub(super) fn save_settings(&mut self, cx: &mut Context<Self>) {
        self.settings.codex_model = self.codex_model.read(cx).value().to_string().trim().into();
        self.settings.claude_model = self.claude_model.read(cx).value().to_string().trim().into();
        self.settings.codex_path = self.codex_path.read(cx).value().to_string().trim().into();
        self.settings.claude_path = self.claude_path.read(cx).value().to_string().trim().into();
        self.submit(Command::Settings(self.settings.clone()), cx);
    }
    fn connection_action(&mut self, provider: ProviderKind, login: bool, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.save_settings(cx);
        self.service.cancel.store(false, Ordering::SeqCst);
        self.submit(
            if login {
                Command::Login(provider)
            } else {
                Command::Inspect(provider)
            },
            cx,
        );
    }
    pub(super) fn settings_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.busy();
        let mut body=v_flex().gap_5().child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child("설정")).child(muted("선택한 자료 구절·관련 원장·선행 작업 출력이 구독 provider로 전송됩니다. 인증 정보는 프로젝트와 내보내기에 포함되지 않습니다.",cx))
            .child(v_flex().gap_2().child("기본 구독").child(RadioGroup::new("primary-provider").children(["Codex · ChatGPT 구독","Claude · Claude Code 구독"]).selected_index(Some(if self.settings.primary==ProviderKind::Codex{0}else{1})).on_change(cx.listener(|this,ix,_,cx|{if this.busy(){return;}this.settings.primary=if *ix==0{ProviderKind::Codex}else{ProviderKind::Claude};this.save_settings(cx);}))))
            .child(Checkbox::new("cross-review").label("비판 검토에 다른 구독 사용 (두 계정 연결 필요)").checked(self.settings.cross_review).disabled(busy).on_change(cx.listener(|this,value,_,cx|{this.settings.cross_review = *value;this.save_settings(cx);})))
            .child(muted("문헌 검색은 PubMed와 ScholarPen에 설정된 TinyFish 연결을 사용합니다. 질문에서 만든 검색어가 검색 서비스에 전달되며, 검색 사용량은 해당 서비스 정책을 따릅니다.",cx)).child(muted("API 결제로 자동 전환하지 않습니다. 계정 사용량은 다른 앱과 공유하며, 남은 사용량을 알 수 없으면 ‘알 수 없음’으로 표시합니다.",cx));
        for provider in [ProviderKind::Codex, ProviderKind::Claude] {
            let name = provider.label();
            let c = self.connections.iter().find(|c| c.provider == name);
            let (model, path) = if provider == ProviderKind::Codex {
                (&self.codex_model, &self.codex_path)
            } else {
                (&self.claude_model, &self.claude_path)
            };
            let mut section = v_flex()
                .gap_3()
                .pt_4()
                .border_t_1()
                .border_color(cx.theme().border)
                .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if c.is_some_and(|c| c.authenticated) {
                            cx.theme().success
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(if c.is_some_and(|c| c.authenticated) {
                            "연결됨 · 구독 인증 확인"
                        } else {
                            if c.is_none() { "연결 상태 조회 중" } else { "구독 연결 필요" }
                        }),
                )
                .child(muted(
                    c.map(|c| c.message.clone())
                        .unwrap_or_else(|| "설치된 CLI의 구독 인증을 확인합니다. 실행 중이라면 완료 후 연결 확인을 누르세요.".into()),
                    cx,
                ))
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new(SharedString::from(format!("inspect-{name}")))
                                .label("연결 확인")
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.connection_action(provider, false, cx)
                                })),
                        )
                        .child(
                            Button::new(SharedString::from(format!("login-{name}")))
                                .label("구독 로그인…")
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.connection_action(provider, true, cx)
                                })),
                        ),
                )
                .child(
                    h_flex().child(
                        Button::new(SharedString::from(format!("probe-{name}")))
                            .small()
                            .label("응답 테스트 · 구독 호출 1회")
                            .disabled(busy || !c.is_some_and(|c| c.authenticated))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.save_settings(cx);
                                this.service.cancel.store(false, Ordering::SeqCst);
                                this.submit(Command::Probe(provider), cx);
                            })),
                    ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child("모델 (비워 두면 계정 기본값)")
                        .child(
                            Input::new(model)
                                .id(SharedString::from(format!("model-{name}")))
                                .disabled(busy),
                        ),
                )
                .when(self.show_advanced, |v| {
                    v.child(
                        v_flex()
                            .gap_2()
                            .child("CLI 실행 파일 (비워 두면 자동 탐지)")
                            .child(
                                Input::new(path)
                                    .id(SharedString::from(format!("path-{name}")))
                                    .disabled(busy),
                            ),
                    )
                });
            if let Some(c) = c {
                section = section.when(self.show_advanced, |v| {
                    v.child(muted(format!("실행 파일: {}", c.executable), cx))
                });
                let mut options = h_flex().flex_wrap().gap_1();
                for slug in &c.models {
                    let slug = slug.clone();
                    let label = slug.clone();
                    options = options.child(
                        Button::new(SharedString::from(format!("model-option-{slug}")))
                            .ghost()
                            .small()
                            .justify_start()
                            .label(label)
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, w, cx| {
                                let field = if provider == ProviderKind::Codex {
                                    &this.codex_model
                                } else {
                                    &this.claude_model
                                };
                                field.update(cx, |s, cx| s.set_value(slug.clone(), w, cx));
                                this.save_settings(cx);
                                cx.notify();
                            })),
                    );
                }
                section = section.child(options);
                section = section.child(document(
                    SharedString::from(format!("quota-{name}")),
                    if let Some(q) = &c.allowance {
                        format!(
                            "구독 포함 사용 허용: {}\n\n{}",
                            match q["ordinaryUsageAllowed"].as_bool() {
                                Some(true) => "예",
                                Some(false) => "아니요",
                                None => "알 수 없음",
                            },
                            quota_text(q)
                        )
                    } else {
                        "남은 구독 사용량: 알 수 없음".into()
                    },
                ));
            }
            if provider == ProviderKind::Codex {
                section = section.child(
                    Button::new("logout-codex")
                        .ghost()
                        .small()
                        .label("이 앱에서 로그아웃")
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.service.cancel.store(false, Ordering::SeqCst);
                            this.submit(Command::LogoutCodex, cx);
                        })),
                )
            } else {
                section=section.child(muted("Claude Code의 기존 구독 로그인을 사용합니다. 계정의 추가 사용 설정은 Claude에서 관리합니다.",cx));
            }
            body = body.child(section);
        }
        body = body.child(
            v_flex()
                .gap_2()
                .child("프로젝트 제목")
                .child(Input::new(&self.new_title).id("rename-project-title"))
                .child(
                    Button::new("rename-project")
                        .label("이름 변경")
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.submit(
                                Command::Rename(this.new_title.read(cx).value().to_string()),
                                cx,
                            );
                        })),
                ),
        );
        body = body.child(
            Button::new("advanced-connection")
                .ghost()
                .label(if self.show_advanced {
                    "고급 연결 설정 닫기"
                } else {
                    "고급 연결 설정"
                })
                .on_click(cx.listener(|this, _, _, cx| {
                    this.show_advanced = !this.show_advanced;
                    cx.notify();
                })),
        );
        body = body
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Checkbox::new("dark-mode")
                            .label("다크 모드")
                            .checked(self.settings.dark)
                            .on_change(cx.listener(|this, v, w, cx| {
                                this.settings.dark = *v;
                                Theme::change(
                                    if *v {
                                        ThemeMode::Dark
                                    } else {
                                        ThemeMode::Light
                                    },
                                    Some(w),
                                    cx,
                                );
                                this.layout_changed = Some(Instant::now());
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("zoom-out")
                            .small()
                            .label("글자 −")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.zoom =
                                    this.settings.zoom.max(16).saturating_sub(1).max(12);
                                Theme::update(cx, |t| t.font_size = px(this.settings.zoom as f32));
                                this.layout_changed = Some(Instant::now());
                            })),
                    )
                    .child(
                        Button::new("zoom-in")
                            .small()
                            .label("글자 +")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.settings.zoom = (this.settings.zoom.max(16) + 1).min(22);
                                Theme::update(cx, |t| t.font_size = px(this.settings.zoom as f32));
                                this.layout_changed = Some(Instant::now());
                            })),
                    ),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(
                        Button::new("save-settings")
                            .primary()
                            .label("연결 설정 저장")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.save_settings(cx))),
                    )
                    .child(
                        Button::new("import-bundle")
                            .label("프로젝트 bundle 가져오기…")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.choose_path("bundle", cx))),
                    ),
            );
        body.into_any_element()
    }
}
fn quota_text(q: &serde_json::Value) -> String {
    let buckets: Vec<_> = q["rateLimitsByLimitId"]
        .as_object()
        .map(|m| m.values().collect())
        .unwrap_or_else(|| vec![&q["rateLimits"]]);
    let mut rows = vec![];
    for b in buckets {
        for key in ["primary", "secondary"] {
            if let Some(n) = b[key]["usedPercent"].as_f64() {
                let reset = b[key]["resetsAt"]
                    .as_u64()
                    .map(|t| {
                        let minutes = t.saturating_sub(theory_core::now()).div_ceil(60);
                        if minutes == 0 {
                            "사용량 갱신 예정".into()
                        } else if minutes < 60 {
                            format!("약 {minutes}분 후 재설정")
                        } else if minutes < 1440 {
                            format!("약 {}시간 후 재설정", minutes.div_ceil(60))
                        } else {
                            format!("약 {}일 후 재설정", minutes.div_ceil(1440))
                        }
                    })
                    .unwrap_or_else(|| "재설정 시각 알 수 없음".into());
                rows.push(format!(
                    "- {} · {}: {:.0}% 사용 · {}",
                    b["limitName"].as_str().unwrap_or("계정"),
                    if key == "primary" {
                        "기본 한도"
                    } else {
                        "추가 기간 한도"
                    },
                    n,
                    reset
                ));
            }
        }
    }
    if rows.is_empty() {
        "상세 사용량: 알 수 없음".into()
    } else {
        rows.join("\n")
    }
}
