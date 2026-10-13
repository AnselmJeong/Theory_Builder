use std::{
    fs,
    io::{Cursor, Write},
    sync::{Arc, atomic::AtomicBool},
};
use theory_engine::{providers::Cancel, storage::*};
fn cancel() -> Cancel {
    Arc::new(AtomicBool::new(false))
}
#[test]
fn folder_sources_preserve_versions_restore_and_reopen() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("Scholar project");
    fs::create_dir_all(folder.join("resources/articles")).unwrap();
    let file = folder.join("resources/articles/paper.md");
    fs::write(&file, "original quote").unwrap();
    let mut store = Store::open_folder(root.path(), &folder).unwrap();
    assert!(store.sync_resources(&cancel(), &mut |_| {}).unwrap());
    let original = store.project.sources[0].id.clone();
    assert!(!store.sync_resources(&cancel(), &mut |_| {}).unwrap());
    fs::write(&file, "changed quote longer").unwrap();
    store.sync_resources(&cancel(), &mut |_| {}).unwrap();
    assert_eq!(store.project.sources.len(), 2);
    assert!(!store.project.sources[0].active);
    assert!(store.project.sources[1].active);
    assert_eq!(store.project.sources[0].text, "original quote");
    fs::write(&file, "original quote").unwrap();
    store.sync_resources(&cancel(), &mut |_| {}).unwrap();
    assert!(store.project.sources[0].active);
    assert!(!store.project.sources[1].active);
    fs::remove_file(&file).unwrap();
    store.sync_resources(&cancel(), &mut |_| {}).unwrap();
    assert!(store.project.sources.iter().all(|s| !s.active));
    store.project.research_guidance = Some("Compare evolutionary and social explanations".into());
    store.save("direction").unwrap();
    let key = store.project.selection_key();
    drop(store);
    let mut reopened = Store::open(root.path(), &key).unwrap();
    assert!(reopened.project.guidance().contains("evolutionary"));
    fs::write(&file, "original quote").unwrap();
    reopened.sync_resources(&cancel(), &mut |_| {}).unwrap();
    assert_eq!(
        reopened.project.sources.iter().filter(|s| s.active).count(),
        1
    );
    assert_eq!(reopened.project.sources[0].id, original);
    assert!(
        Store::open(root.path(), &key).is_err(),
        "second writer must be rejected"
    );
    let bundle = reopened.export(&root.path().join("export")).unwrap();
    let copy = Store::import_bundle(&root.path().join("copy"), &bundle).unwrap();
    assert!(
        copy.project.workspace_folder.is_none(),
        "portable copy must not follow original folder"
    );
}
#[test]
fn cancelled_sync_rolls_back_and_next_sync_retries() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("project");
    fs::create_dir_all(folder.join("resource")).unwrap();
    fs::write(folder.join("resource/a.txt"), "test").unwrap();
    let mut store = Store::open_folder(root.path(), &folder).unwrap();
    let token = cancel();
    let result = store.sync_resources(&token, &mut |_| {
        token.store(true, std::sync::atomic::Ordering::SeqCst)
    });
    assert!(result.is_err());
    assert!(store.project.sources.is_empty());
    assert!(store.sync_resources(&cancel(), &mut |_| {}).unwrap());
}
fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, text) in entries {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
#[test]
fn epub_and_docx_are_read_in_document_order() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("project");
    fs::create_dir_all(folder.join("resources/books")).unwrap();
    fs::write(folder.join("resources/books/book.epub"),archive(&[
        ("META-INF/container.xml","<container><rootfiles><rootfile full-path='OPS/content.opf'/></rootfiles></container>"),
        ("OPS/content.opf","<package><manifest><item id='a' href='a.xhtml'/><item id='b' href='b.xhtml'/></manifest><spine><itemref idref='b'/><itemref idref='a'/></spine></package>"),
        ("OPS/a.xhtml","<html><head><title>Ignore</title></head><body><p>Second <em>section</em>.</p></body></html>"),
        ("OPS/b.xhtml","<html><body><p>First section.</p><script>Ignore script</script></body></html>")
    ])).unwrap();
    fs::write(folder.join("resources/books/note.docx"),archive(&[("word/document.xml","<document><body><p><r><t>First paragraph</t></r></p><p><r><t>Second paragraph</t></r></p></body></document>")])).unwrap();
    let mut store = Store::open_folder(root.path(), &folder).unwrap();
    store.sync_resources(&cancel(), &mut |_| {}).unwrap();
    let book = store
        .project
        .sources
        .iter()
        .find(|s| s.extension == "epub")
        .unwrap();
    assert!(book.text.find("First").unwrap() < book.text.find("Second").unwrap());
    assert!(!book.text.contains("Ignore"));
    assert!(
        store
            .project
            .sources
            .iter()
            .find(|s| s.extension == "docx")
            .unwrap()
            .text
            .contains("First paragraph\nSecond paragraph")
    );
}
#[test]
fn local_project_moves_to_shared_folder_without_losing_history() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::create(root.path(), "Existing research").unwrap();
    store.project.draft = "User's original question".into();
    store.project.checkpoint("original");
    store.save("original").unwrap();
    let shared = store
        .share(root.path(), &root.path().join("Scholar project"))
        .unwrap();
    assert_eq!(shared.project.id, store.project.id);
    assert_eq!(shared.project.draft, store.project.draft);
    assert_eq!(shared.project.history.len(), store.project.history.len());
    assert!(root.path().join("Scholar project/resources/books").is_dir());
}
#[test]
fn deleted_shared_file_can_be_reimported() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("shared");
    fs::create_dir_all(folder.join("resources/articles")).unwrap();
    let source = root.path().join("paper.txt");
    fs::write(&source, "Recover this quote").unwrap();
    let mut store = Store::open_folder(root.path(), &folder).unwrap();
    store.import_file(&source).unwrap();
    fs::remove_file(folder.join("resources/articles/paper.txt")).unwrap();
    store.sync_resources(&cancel(), &mut |_| {}).unwrap();
    assert!(!store.project.sources[0].active);
    store.import_file(&source).unwrap();
    assert!(folder.join("resources/articles/paper.txt").exists());
    assert!(store.project.sources[0].active);
}

#[test]
fn typed_legacy_asset_recovers_only_when_hash_matches() {
    let root = tempfile::tempdir().unwrap();
    let folder = root.path().join("shared");
    fs::create_dir_all(&folder).unwrap();
    let mut store = Store::open_folder(root.path(), &folder).unwrap();
    let bytes = b"Old document content";
    let digest = theory_core::hash(bytes);
    let mut source = theory_core::Source::new(
        "Old document".into(),
        String::new(),
        "old.txt".into(),
        theory_core::Access::MetadataOnly,
    );
    source.asset_hash = Some(digest.clone());
    source.extension = "txt".into();
    store.project.sources.push(source);
    let typed = folder
        .join(".theory-studio/assets")
        .join(format!("{digest}.txt"));
    fs::write(&typed, b"wrong bytes").unwrap();
    assert!(store.sync_resources(&cancel(), &mut |_| {}).is_err());
    assert!(store.project.sources[0].text.is_empty());
    fs::write(&typed, bytes).unwrap();
    assert!(store.sync_resources(&cancel(), &mut |_| {}).unwrap());
    assert_eq!(store.project.sources[0].text, "Old document content");
    assert!(
        store
            .source_path(&store.project.sources[0].id)
            .unwrap()
            .exists()
    );
    store.export(&root.path().join("export")).unwrap();
}

#[test]
fn export_uses_stable_extensions_and_reads_legacy_extensionless_bundles() {
    let root = tempfile::tempdir().unwrap();
    let mut store = Store::create(root.path(), "Export").unwrap();
    let file = root.path().join("paper.txt");
    fs::write(&file, "Stable text").unwrap();
    store.import_file(&file).unwrap();
    let source = &store.project.sources[0];
    let digest = source.asset_hash.as_ref().unwrap();
    let bundle = store.export(&root.path().join("export")).unwrap();
    let typed = bundle.join("assets").join(format!("{digest}.txt"));
    assert!(typed.is_file());
    Store::import_bundle(&root.path().join("typed-copy"), &bundle).unwrap();
    fs::rename(&typed, bundle.join("assets").join(digest)).unwrap();
    let manifest = bundle.join("manifest.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["version"] = 1.into();
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    Store::import_bundle(&root.path().join("legacy-copy"), &bundle).unwrap();
    assert!(
        !typed.exists(),
        "reading a legacy bundle must not modify it"
    );
}
