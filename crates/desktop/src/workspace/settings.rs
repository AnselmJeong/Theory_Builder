use super::*;
use gpui_kit::component::radio::RadioGroup;
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
        let pairings = [
            (ProviderKind::Claude, true, "Claude 구성 · GPT 비판"),
            (ProviderKind::Codex, true, "GPT 구성 · Claude 비판"),
            (ProviderKind::Claude, false, "Claude 단독 · 별도 문맥 비판"),
            (ProviderKind::Codex, false, "GPT 단독 · 별도 문맥 비판"),
        ];
        let selected = pairings
            .iter()
            .position(|(p, x, _)| *p == self.settings.primary && *x == self.settings.cross_review);
        let mut body = v_flex()
            .gap_5()
            .child(page_header(
                "설정",
                "선택한 자료 구절·관련 원장·이전 단계 출력이 구독 모델로 전송됩니다. 인증 정보는 프로젝트와 내보내기에 포함되지 않습니다.",
                cx,
            ))
            .child(
                card(cx)
                    .gap_3()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("토론 구성"))
                    .child(muted(
                        "구성 모델이 계획·초안·수정·종합을 맡고, 비판 모델이 비판과 판정을 맡습니다. 비판의 해소 여부는 비판 모델만 판정합니다. 두 모델의 합의는 경험적 근거가 아닙니다.",
                        cx,
                    ))
                    .child(
                        RadioGroup::new("pairing")
                            .children(pairings.iter().map(|(_, _, l)| *l))
                            .selected_index(selected)
                            .on_change(cx.listener(move |this, ix: &usize, _, cx| {
                                if this.busy() {
                                    return;
                                }
                                let (p, x, _) = pairings[*ix];
                                this.settings.primary = p;
                                this.settings.cross_review = x;
                                this.save_settings(cx);
                            })),
                    ),
            )
            .child(div().pt_2().font_weight(FontWeight::SEMIBOLD).child("모델 연결"));
        for provider in [ProviderKind::Claude, ProviderKind::Codex] {
            body = body.child(self.connection_card(provider, cx));
        }
        body = body
            .child(
                card(cx)
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(div().flex_1().font_weight(FontWeight::SEMIBOLD).child("문헌 검색"))
                            .child(
                                Switch::new("settings-search")
                                    .checked(self.settings.search_enabled)
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, v: &bool, _, cx| {
                                        this.settings.search_enabled = *v;
                                        this.save_settings(cx);
                                    })),
                            ),
                    )
                    .child(muted(
                        "계획 단계에서 만든 검색어(최대 2개)로 PubMed 초록과, ScholarPen에 TinyFish 연결이 있으면 웹 발췌를 가져옵니다. 검색 결과의 제목만으로는 근거로 쓰지 않습니다.",
                        cx,
                    )),
            )
            .child(
                card(cx)
                    .gap_3()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("화면"))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(
                                Switch::new("dark-mode")
                                    .label("다크 모드")
                                    .checked(self.settings.dark)
                                    .on_click(cx.listener(|this, v: &bool, w, cx| {
                                        this.settings.dark = *v;
                                        Theme::change(
                                            if *v { ThemeMode::Dark } else { ThemeMode::Light },
                                            Some(w),
                                            cx,
                                        );
                                        this.layout_changed = Some(Instant::now());
                                        cx.notify();
                                    })),
                            )
                            .child(div().flex_1())
                            .child(muted("글자 크기", cx))
                            .child(
                                Button::new("zoom-out")
                                    .small()
                                    .icon(Icon::new(Lucide::Minus))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings.zoom =
                                            this.settings.zoom.max(16).saturating_sub(1).max(12);
                                        Theme::update(cx, |t| t.font_size = px(this.settings.zoom as f32));
                                        this.layout_changed = Some(Instant::now());
                                    })),
                            )
                            .child(div().text_sm().child(format!("{}", self.settings.zoom.max(16))))
                            .child(
                                Button::new("zoom-in")
                                    .small()
                                    .icon(Icon::new(Lucide::Plus))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.settings.zoom = (this.settings.zoom.max(16) + 1).min(22);
                                        Theme::update(cx, |t| t.font_size = px(this.settings.zoom as f32));
                                        this.layout_changed = Some(Instant::now());
                                    })),
                            ),
                    ),
            )
            .child(
                card(cx)
                    .gap_3()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("프로젝트"))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.new_title).id("rename-project-title")))
                            .child(
                                Button::new("rename-project")
                                    .small()
                                    .label("이름 변경")
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.submit(
                                            Command::Rename(this.new_title.read(cx).value().to_string()),
                                            cx,
                                        );
                                    })),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("settings-export")
                                    .small()
                                    .icon(Icon::new(Lucide::Share))
                                    .label("내보내기…")
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.choose_path("export", cx))),
                            )
                            .child(
                                Button::new("settings-import-bundle")
                                    .small()
                                    .ghost()
                                    .label("bundle 가져오기…")
                                    .disabled(busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.choose_path("bundle", cx))),
                            ),
                    ),
            );
        body.into_any_element()
    }
    fn connection_card(&self, provider: ProviderKind, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let name = provider.label();
        let c = self.connection(provider);
        let (model, path) = if provider == ProviderKind::Codex {
            (&self.codex_model, &self.codex_path)
        } else {
            (&self.claude_model, &self.claude_path)
        };
        let (status, color) = match c {
            Some(c) if c.authenticated => ("연결됨", cx.theme().success),
            Some(c) if !c.installed => ("CLI 없음", cx.theme().danger),
            Some(_) => ("로그인 필요", gpui_kit::component::amber_500()),
            None => ("확인 중", cx.theme().muted_foreground),
        };
        let mut section = card(cx)
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(avatar(name, cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(provider.display()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(if provider == ProviderKind::Codex {
                                        "Codex CLI · 이 앱 전용 ChatGPT 구독 로그인"
                                    } else {
                                        "Claude Code CLI · 기존 Claude 구독 로그인"
                                    }),
                            ),
                    )
                    .child(chip(status, color)),
            )
            .child(muted(
                c.map(|c| c.message.clone())
                    .unwrap_or_else(|| "설치된 CLI의 구독 인증을 확인하고 있습니다.".into()),
                cx,
            ))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new(SharedString::from(format!("inspect-{name}")))
                            .small()
                            .icon(Icon::new(Lucide::RefreshCw))
                            .label("연결 확인")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.connection_action(provider, false, cx)
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("login-{name}")))
                            .small()
                            .label("구독 로그인…")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.connection_action(provider, true, cx)
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("probe-{name}")))
                            .small()
                            .ghost()
                            .label("응답 테스트 (호출 1회)")
                            .disabled(busy || !c.is_some_and(|c| c.authenticated))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.save_settings(cx);
                                this.service.cancel.store(false, Ordering::SeqCst);
                                this.submit(Command::Probe(provider), cx);
                            })),
                    )
                    .when(provider == ProviderKind::Codex, |v| {
                        v.child(
                            Button::new("logout-codex")
                                .ghost()
                                .small()
                                .label("로그아웃")
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.service.cancel.store(false, Ordering::SeqCst);
                                    this.submit(Command::LogoutCodex, cx);
                                })),
                        )
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().w(rems(4.)).text_sm().child("모델"))
                    .child(
                        div().flex_1().child(
                            Input::new(model)
                                .id(SharedString::from(format!("model-{name}")))
                                .disabled(busy),
                        ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("save-model-{name}")))
                            .small()
                            .label("저장")
                            .disabled(busy)
                            .on_click(cx.listener(|this, _, _, cx| this.save_settings(cx))),
                    ),
            );
        if let Some(c) = c {
            if !c.models.is_empty() {
                let mut options = h_flex().flex_wrap().gap_1();
                for slug in &c.models {
                    let slug = slug.clone();
                    let current = self.settings.model(provider) == slug;
                    options = options.child(
                        Button::new(SharedString::from(format!("model-option-{slug}")))
                            .ghost()
                            .xsmall()
                            .selected(current)
                            .label(slug.clone())
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
            }
            section = section.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(match &c.allowance {
                        Some(q) => format!(
                            "구독 포함 사용: {} · {}",
                            match q["ordinaryUsageAllowed"].as_bool() {
                                Some(true) => "허용",
                                Some(false) => "허용 안 됨",
                                None => "알 수 없음",
                            },
                            quota_text(q).replace('\n', " · ")
                        ),
                        None => "남은 구독 사용량: 알 수 없음 (자동 유료 전환 없음)".into(),
                    }),
            );
        }
        section
            .child(
                h_flex().child(
                    Button::new(SharedString::from(format!("advanced-{name}")))
                        .ghost()
                        .xsmall()
                        .icon(Icon::new(if self.show_advanced {
                            Lucide::ChevronDown
                        } else {
                            Lucide::ChevronRight
                        }))
                        .label("고급 설정")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_advanced = !this.show_advanced;
                            cx.notify();
                        })),
                ),
            )
            .when(self.show_advanced, |v| {
                v.child(
                    v_flex()
                        .gap_2()
                        .child(muted("CLI 실행 파일 (비워 두면 자동 탐지)", cx))
                        .child(
                            Input::new(path)
                                .id(SharedString::from(format!("path-{name}")))
                                .disabled(busy),
                        )
                        .when_some(c.map(|c| c.executable.clone()), |v, exe| {
                            v.child(muted(format!("현재: {exe}"), cx))
                        }),
                )
            })
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
