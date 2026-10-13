use gpui_kit::{component::TitleBar, *};
use theory_studio::workspace::*;
fn main() {
    application().with_assets(assets::AllAssets).run(|cx| {
        init(cx);
        cx.bind_keys([
            KeyBinding::new("secondary-enter", RunQuestion, Some("TheoryStudio")),
            KeyBinding::new("secondary-n", NewQuestion, Some("TheoryStudio")),
            KeyBinding::new("secondary-shift-n", NewProject, Some("TheoryStudio")),
            KeyBinding::new("secondary-o", ImportSource, Some("TheoryStudio")),
            KeyBinding::new("secondary-e", ExportProject, Some("TheoryStudio")),
            KeyBinding::new("secondary-f", FindSource, Some("TheoryStudio")),
            KeyBinding::new("secondary-,", OpenSettings, Some("TheoryStudio")),
            KeyBinding::new("secondary-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.set_menus(vec![
            Menu {
                name: "Theory Studio".into(),
                items: vec![
                    MenuItem::action("설정…", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("종료", Quit),
                ],
                disabled: false,
            },
            Menu {
                name: "파일".into(),
                items: vec![
                    MenuItem::action("새 질문", NewQuestion),
                    MenuItem::action("새 프로젝트…", NewProject),
                    MenuItem::action("자료 추가…", ImportSource),
                    MenuItem::action("내보내기…", ExportProject),
                ],
                disabled: false,
            },
            Menu {
                name: "연구".into(),
                items: vec![
                    MenuItem::action("질문 실행", RunQuestion),
                    MenuItem::action("자료 검색", FindSource),
                ],
                disabled: false,
            },
        ]);
        // Pixels here describe native window geometry, not product spacing.
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1360.), px(900.)),
                cx,
            ))),
            window_min_size: Some(size(px(1000.), px(700.))),
            app_id: Some("ai.academa.theory-studio".into()),
            ..TitleBar::window_options()
        };
        let (window, view) = open_window(options, cx, |window, cx| {
            window.set_window_title("Theory Studio");
            cx.new(|cx| Workspace::new(window, cx))
        })
        .expect("Theory Studio 창을 열 수 없습니다");
        let weak = view.downgrade();
        cx.on_app_quit(move |cx| {
            let _ = weak.update(cx, |v, cx| v.shutdown(cx));
            let executor = cx.background_executor().clone();
            async move {
                executor.timer(std::time::Duration::from_millis(500)).await;
            }
        })
        .detach();
        cx.on_window_closed(move |cx, id| {
            if id == window.window_id() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
