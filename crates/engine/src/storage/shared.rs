use super::*;
use crate::providers::{
    Cancel,
    process::{Process, isolated_command},
};
use std::collections::HashSet;
use std::time::{Duration, Instant};

pub fn scholarpen_root() -> PathBuf {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("ScholarPen");
    let value: serde_json::Value = fs::read(home.join("settings.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    value["projectsRootDir"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or(home)
}
pub fn shared_folders(root: &Path) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = fs::read(root.join("folders.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    if root == data_directory() {
        // Cloud-provider directory enumeration must never gate local app startup.
        static DISCOVERED: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<Vec<PathBuf>>>> =
            std::sync::OnceLock::new();
        let discovered = DISCOVERED.get_or_init(|| {
            let found = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
            let output = found.clone();
            std::thread::spawn(move || {
                if let Ok(items) = fs::read_dir(scholarpen_root()) {
                    let paths = items
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| p.join("resources").is_dir() || p.join("resource").is_dir())
                        .collect();
                    if let Ok(mut result) = output.lock() {
                        *result = paths;
                    }
                }
            });
            found
        });
        if let Ok(found) = discovered.lock() {
            folders.extend(found.iter().cloned());
        }
    }
    folders.sort();
    folders.dedup();
    folders
}
fn remember(root: &Path, folder: &Path) -> Result<()> {
    let mut folders = shared_folders(root);
    if !folders.contains(&folder.to_path_buf()) {
        folders.push(folder.to_path_buf());
    }
    atomic_write(&root.join("folders.json"), &serde_json::to_vec(&folders)?)
}
impl Store {
    pub fn open_folder(root: &Path, folder: &Path) -> Result<Self> {
        let folder = folder
            .canonicalize()
            .context("프로젝트 폴더를 찾지 못했습니다")?;
        ensure!(folder.is_dir(), "프로젝트 폴더를 선택하세요");
        let directory = folder.join(".theory-studio");
        let p = if directory.join("project.sqlite").exists() {
            let db = Connection::open_with_flags(
                directory.join("project.sqlite"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )?;
            serde_json::from_str::<Project>(&db.query_row::<String, _, _>(
                "SELECT json FROM snapshot WHERE id=1",
                [],
                |r| r.get(0),
            )?)?
        } else {
            Project::new(&folder.file_name().unwrap_or_default().to_string_lossy())
        };
        let mut p = p;
        p.workspace_folder = Some(folder.to_string_lossy().into());
        let store = Self::open_directory(directory, p)?;
        remember(root, &folder)?;
        Ok(store)
    }
    pub fn share(&self, root: &Path, folder: &Path) -> Result<Self> {
        fs::create_dir_all(folder)?;
        ensure!(
            !folder.join(".theory-studio/project.sqlite").exists(),
            "이 폴더에 이미 연구 기록이 있습니다. ‘프로젝트 폴더 열기’를 사용하세요"
        );
        for path in [
            "documents",
            "drafts",
            "resources/articles",
            "resources/books",
            "figures",
            "exports",
        ] {
            fs::create_dir_all(folder.join(path))?;
        }
        let folder = folder.canonicalize()?;
        let directory = folder.join(".theory-studio");
        fs::create_dir_all(directory.join("assets"))?;
        for source in &self.project.sources {
            if let Some(h) = &source.asset_hash {
                ensure!(safe_hash(h), "자산 hash 오류");
                let bytes = read_bounded(&stored_asset(&self.directory, source)?, MAX_ASSET)?;
                ensure!(hash(&bytes) == *h, "공유할 원본 파일의 hash가 다릅니다");
                atomic_write(
                    &directory
                        .join("assets")
                        .join(asset_name(h, &source.extension)),
                    &bytes,
                )?;
            }
        }
        let mut p = self.project.clone();
        p.workspace_folder = Some(folder.to_string_lossy().into());
        let store = Self::open_directory(directory, p)?;
        remember(root, &folder)?;
        Ok(store)
    }
    pub fn sync_resources(
        &mut self,
        cancel: &Cancel,
        progress: &mut dyn FnMut(String),
    ) -> Result<bool> {
        let before = self.project.clone();
        let stamps = self.resource_stamps.clone();
        let result = self.sync_resources_inner(cancel, progress);
        if result.is_err() {
            self.project = before;
            self.resource_stamps = stamps;
        }
        result
    }
    fn sync_resources_inner(
        &mut self,
        cancel: &Cancel,
        progress: &mut dyn FnMut(String),
    ) -> Result<bool> {
        let Some(folder) = self.project.workspace_folder.clone() else {
            return Ok(false);
        };
        let base = PathBuf::from(folder);
        ensure!(
            base.is_dir(),
            "프로젝트 폴더에 접근할 수 없습니다. 기존 자료는 보존했습니다"
        );
        let mut paths = vec![];
        for name in ["resources", "resource"] {
            let dir = base.join(name);
            if dir.is_dir() {
                collect(&dir, &mut paths, 0)?;
            }
        }
        paths.sort();
        paths.dedup();
        let seen: HashSet<_> = paths
            .iter()
            .filter_map(|p| p.strip_prefix(&base).ok())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        let mut changed = false;
        // Upgrade old PDF attachments that the previous version kept as metadata only.
        for source in &mut self.project.sources {
            if !source.text.is_empty() && source.locator.contains(" · 본문 추출 안 됨") {
                source.locator = source.locator.replace(" · 본문 추출 안 됨", "");
                changed = true;
            }
            if source.relative_path.is_none()
                && source.active
                && source.access == Access::MetadataOnly
                && let Some(digest) = &source.asset_hash
            {
                ensure!(safe_hash(digest), "자산 hash 오류");
                let asset = stored_asset(&self.directory, source)?;
                let key = format!("legacy:{}", source.id);
                let meta = fs::metadata(&asset)?;
                let stamp = (meta.len(), meta.modified()?);
                if self.resource_stamps.get(&key) == Some(&stamp) {
                    continue;
                }
                progress(format!("기존 자료 본문 읽는 중 · {}", source.title));
                let bytes = read_bounded(&asset, MAX_ASSET)?;
                ensure!(hash(&bytes) == *digest, "자산이 변경되었습니다");
                let (text, note) = extract(&asset, &source.extension, &bytes, cancel);
                ensure!(
                    !cancel.load(std::sync::atomic::Ordering::SeqCst),
                    "자료 동기화 중단"
                );
                if !text.trim().is_empty() {
                    source.hash = hash(text.as_bytes());
                    source.text = text;
                    source.access = Access::ProvidedText;
                    source.locator = source.locator.replace(" · 본문 추출 안 됨", "");
                    changed = true;
                }
                if source.import_note != note {
                    source.import_note = note;
                    changed = true;
                }
                self.resource_stamps.insert(key, stamp);
            }
        }
        for (ix, path) in paths.iter().enumerate() {
            ensure!(
                !cancel.load(std::sync::atomic::Ordering::SeqCst),
                "자료 동기화 중단"
            );
            let rel = path.strip_prefix(&base)?.to_string_lossy().into_owned();
            let meta = fs::metadata(path)?;
            let stamp = (meta.len(), meta.modified()?);
            if self.resource_stamps.get(&rel) == Some(&stamp) {
                continue;
            }
            progress(format!(
                "자료 읽는 중 {}/{} · {}",
                ix + 1,
                paths.len(),
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
            let bytes = match read_bounded(path, MAX_ASSET) {
                Ok(b) => b,
                Err(e) => {
                    return Err(e.context(format!(
                        "자료를 읽지 못해 동기화를 보류했습니다: {}. 기존 판본은 보존했습니다",
                        path.display()
                    )));
                }
            };
            let digest = hash(&bytes);
            let existing = self.project.sources.iter().position(|s| {
                s.relative_path.as_ref() == Some(&rel) && s.asset_hash.as_ref() == Some(&digest)
            });
            if let Some(index) = existing {
                for (i, source) in self.project.sources.iter_mut().enumerate() {
                    if i != index && source.relative_path.as_ref() == Some(&rel) && source.active {
                        source.active = false;
                        changed = true;
                    }
                }
                if !self.project.sources[index].active {
                    self.project.sources[index].active = true;
                    changed = true;
                }
                if self.project.sources[index].access == Access::MetadataOnly {
                    let (text, note) =
                        extract(path, &self.project.sources[index].extension, &bytes, cancel);
                    let source = &mut self.project.sources[index];
                    if !text.trim().is_empty() {
                        source.hash = hash(text.as_bytes());
                        source.text = text;
                        source.access = Access::ProvidedText;
                        changed = true;
                    }
                    if source.import_note != note {
                        source.import_note = note;
                        changed = true;
                    }
                }
            } else {
                let ext = path
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                let (text, note) = extract(path, &ext, &bytes, cancel);
                ensure!(
                    !cancel.load(std::sync::atomic::Ordering::SeqCst),
                    "자료 동기화 중단"
                );
                let mut source = Source::new(
                    path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    text,
                    rel.clone(),
                    Access::ProvidedText,
                );
                if source.text.trim().is_empty() {
                    source.access = Access::MetadataOnly;
                }
                source.relative_path = Some(rel.clone());
                source.import_note = note;
                source.asset_hash = Some(digest.clone());
                source.extension = ext;
                source.revision = self
                    .project
                    .sources
                    .iter()
                    .filter(|s| s.relative_path.as_ref() == Some(&rel))
                    .map(|s| s.revision)
                    .max()
                    .unwrap_or(0)
                    + 1;
                atomic_write(
                    &self
                        .directory
                        .join("assets")
                        .join(asset_name(&digest, &source.extension)),
                    &bytes,
                )?;
                for old in &mut self.project.sources {
                    if old.relative_path.as_ref() == Some(&rel) {
                        old.active = false;
                    }
                }
                self.project.sources.push(source);
                changed = true;
            }
            self.resource_stamps.insert(rel, stamp);
        }
        for s in &mut self.project.sources {
            if s.active && s.relative_path.as_ref().is_some_and(|r| !seen.contains(r)) {
                s.active = false;
                changed = true;
            }
        }
        self.resource_stamps
            .retain(|key, _| key.starts_with("legacy:") || seen.contains(key));
        if changed {
            let inactive: HashSet<_> = self
                .project
                .sources
                .iter()
                .filter(|s| !s.active)
                .map(|s| s.id.clone())
                .collect();
            let mut affected: HashSet<_> = self
                .project
                .runs
                .iter()
                .flat_map(|r| &r.attempts)
                .filter_map(|a| a.output.as_ref())
                .flat_map(|o| &o.evidence_refs)
                .filter(|e| inactive.contains(&e.source_id))
                .map(|e| e.claim_id.clone())
                .collect();
            loop {
                let before = affected.len();
                for e in &self.project.entries {
                    if e.proposal.depends_on.iter().any(|d| affected.contains(d)) {
                        affected.insert(e.proposal.id.clone());
                    }
                }
                if before == affected.len() {
                    break;
                }
            }
            for e in &mut self.project.entries {
                if affected.contains(&e.proposal.id) {
                    e.review = Review::NeedsReview;
                }
            }
            self.project
                .checkpoint("ScholarPen 자료 동기화 — 이전 원문 보존");
            self.save("공유 폴더 자료 갱신")?;
        }
        Ok(changed)
    }
}
fn collect(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    ensure!(depth <= 20 && out.len() < 10000, "자료 폴더가 너무 큽니다");
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        let t = entry.file_type()?;
        if entry.file_name().to_string_lossy().starts_with('.') || t.is_symlink() {
            continue;
        }
        if t.is_dir() {
            collect(&p, out, depth + 1)?;
        } else if p.extension().is_some_and(|e| {
            ["pdf", "md", "txt", "qmd", "epub", "docx"]
                .contains(&e.to_string_lossy().to_lowercase().as_str())
        }) {
            out.push(p);
        }
    }
    Ok(())
}
pub(super) fn extract(path: &Path, ext: &str, bytes: &[u8], cancel: &Cancel) -> (String, String) {
    if ["md", "txt", "qmd"].contains(&ext) {
        return match String::from_utf8(bytes.to_vec()) {
            Ok(s) => (s, "텍스트 읽기 완료".into()),
            Err(_) => (String::new(), "UTF-8 텍스트가 아닙니다".into()),
        };
    }
    if ["epub", "docx"].contains(&ext) {
        return super::book::extract(bytes, ext)
            .unwrap_or_else(|e| (String::new(), format!("본문 추출 실패: {e}")));
    }
    if ext == "pdf" {
        let result = (|| -> Result<(String, String)> {
            let exe = std::env::current_exe()?;
            let bundled = exe
                .parent()
                .unwrap()
                .join("../Resources/bin/theory-extract");
            let helper = if bundled.is_file() {
                bundled
            } else {
                PathBuf::from(env!("THEORY_EXTRACT_BUILD"))
            };
            let mut cmd = isolated_command(&helper, path.parent().unwrap());
            cmd.arg(path);
            let raw = Process::text(cmd, cancel, Instant::now() + Duration::from_secs(40), None)?;
            let v: serde_json::Value = serde_json::from_str(&raw)?;
            Ok((
                v["text"].as_str().unwrap_or_default().into(),
                v["note"].as_str().unwrap_or_default().into(),
            ))
        })();
        return result.unwrap_or_else(|e| (String::new(), format!("본문 추출 실패: {e}")));
    }
    (String::new(), "본문 추출 미지원 · 원본 열기 가능".into())
}
