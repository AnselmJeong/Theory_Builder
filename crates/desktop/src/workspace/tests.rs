use super::Workspace;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{AppContext, TestAppContext, component::Root, px, size};

#[gpui_kit::test]
fn real_controls_create_project_and_preserve_korean_input(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let dir = tempfile::tempdir().unwrap();
    let mut entity = None;
    let window = cx.open_window(size(px(1360.), px(900.)), |window, cx| {
        let view = cx.new(|cx| Workspace::with_directory(dir.path().to_path_buf(), window, cx));
        entity = Some(view.clone());
        Root::new(view, window, cx)
    });
    let view = entity.unwrap();
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.update_window(window.into(), |_, w, cx| {
            view.update(cx, |v, cx| v.poll(w, cx))
        })
        .unwrap();
        if cx.update(|cx| view.read(cx).snapshot.is_some()) {
            break;
        }
    }
    cx.update_window(window.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("new-project", cx);
        w.click("project-title", cx);
        w.input("시냅스 가소성 연구", cx);
        w.click("create-project", cx);
    })
    .unwrap();
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.update_window(window.into(), |_, w, cx| {
            view.update(cx, |v, cx| v.poll(w, cx))
        })
        .unwrap();
        if cx.update(|cx| view.read(cx).pending == 0) {
            break;
        }
    }
    cx.update(|cx| {
        assert_eq!(
            view.read(cx).snapshot.as_ref().unwrap().project.title,
            "시냅스 가소성 연구"
        )
    });
    let first_id = cx.update(|cx| view.read(cx).snapshot.as_ref().unwrap().project.id.clone());
    cx.update_window(window.into(), |_, w, cx| {
        w.render_frame(cx);
        w.click("example-concept", cx);
        assert!(
            view.read(cx)
                .prompt
                .read(cx)
                .value()
                .contains("Metastability")
        );
        let prompt_id = view.read(cx).prompt.entity_id();
        w.click(("input", prompt_id), cx);
        w.press("secondary-a", cx);
        w.input("세포와 회로를 구분해 줘", cx);
        w.press("enter", cx);
        assert!(
            view.read(cx)
                .prompt
                .read(cx)
                .value()
                .contains("세포와 회로")
        );
        assert!(
            view.read(cx)
                .snapshot
                .as_ref()
                .unwrap()
                .project
                .runs
                .is_empty(),
            "ordinary Enter must not submit a model call"
        );
        w.click("inspector-toggle", cx);
        assert!(view.read(cx).show_inspector);
        w.click("close-inspector", cx);
        assert!(!view.read(cx).show_inspector);
    })
    .unwrap();
    // Switch immediately, before the autosave debounce. The original question must survive.
    cx.update_window(window.into(), |_, _, cx| {
        view.update(cx, |v, cx| {
            v.submit(
                theory_engine::service::Command::NewProject("Another project".into()),
                cx,
            )
        });
    })
    .unwrap();
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        cx.update_window(window.into(), |_, w, cx| {
            view.update(cx, |v, cx| v.poll(w, cx))
        })
        .unwrap();
        if cx.update(|cx| view.read(cx).pending == 0) {
            break;
        }
    }
    let saved = theory_engine::storage::Store::open(dir.path(), &first_id).unwrap();
    assert!(saved.project.draft.contains("세포와 회로"));
}
