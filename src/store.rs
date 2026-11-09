//! The user's files in the data folder: the style guide, the legacy migration and the log.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const STYLE_TEMPLATE: &str = r#"# My reply style

Describe how you write. This file is read for every Draft reply, so edits apply immediately.

## About me
- Role: (for example, backend engineer)

## Voice
- Direct and friendly, with no filler.
- Simple, common words, like everyday chat.
- Short messages, usually one to three sentences.

## Examples of how I write
- "Sure, I'll take a look this afternoon."
- "Good catch, I'll push a fix today."
"#;

/// The user's editable style guide for drafts.
pub struct StyleStore {
    pub path: PathBuf,
}

impl StyleStore {
    pub fn install_template(&self) -> std::io::Result<()> {
        if self.path.exists() {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(&self.path, STYLE_TEMPLATE)
    }

    pub fn load(&self) -> Option<String> {
        fs::read_to_string(&self.path).ok()
    }
}

/// The app was called TextActions, and its data folder was named after it.
pub const LEGACY_DATA_FOLDER: &str = "TextActions";
const USER_FILES: [&str; 2] = ["settings.json", "style.md"];

#[derive(Debug, Default, PartialEq)]
pub struct Migration {
    pub copied: Vec<&'static str>,
    pub failed: Vec<&'static str>,
}

/// Copies the user's own files from the old data folder, leaving the originals in
/// place. A file already in the new folder wins, so this is safe on every launch.
/// Reports failures rather than returning an error, because the app can start without them.
pub fn migrate_legacy_data(legacy_dir: &Path, data_dir: &Path) -> Migration {
    let mut result = Migration::default();
    for file in USER_FILES {
        let source = legacy_dir.join(file);
        let target = data_dir.join(file);
        if !source.exists() || target.exists() {
            continue;
        }
        let copy = fs::create_dir_all(data_dir).and_then(|_| {
            let mut out = fs::OpenOptions::new().write(true).create_new(true).open(&target)?;
            out.write_all(&fs::read(&source)?)
        });
        match copy {
            Ok(()) => result.copied.push(file),
            Err(_) => result.failed.push(file),
        }
    }
    result
}

/// Records event names and text lengths for support. Never pass selected text or results.
pub fn log_event(log_path: &Path, event: &str, details: &[(&str, String)]) {
    let mut line = serde_json::Map::new();
    line.insert("time".into(), humantime::format_rfc3339_millis(std::time::SystemTime::now()).to_string().into());
    line.insert("event".into(), event.into());
    for (key, value) in details {
        line.insert((*key).into(), value.clone().into());
    }
    // Diagnostics must never break the app.
    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .and_then(|mut file| writeln!(file, "{}", serde_json::Value::Object(line)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn style_creates_the_template_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let store = StyleStore { path: dir.path().join("nested").join("style.md") };
        store.install_template().unwrap();
        assert!(store.load().is_some_and(|s| !s.is_empty()));
    }

    #[test]
    fn style_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let store = StyleStore { path: dir.path().join("style.md") };
        store.install_template().unwrap();
        fs::write(&store.path, "mine").unwrap();
        store.install_template().unwrap();
        assert_eq!(store.load().as_deref(), Some("mine"));
    }

    #[test]
    fn style_is_none_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(StyleStore { path: dir.path().join("style.md") }.load(), None);
    }

    fn dirs() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let legacy = root.path().join("legacy");
        let data = root.path().join("data");
        fs::create_dir(&legacy).unwrap();
        (root, legacy, data)
    }

    #[test]
    fn migration_copies_settings_and_style_and_keeps_originals() {
        let (_root, legacy, data) = dirs();
        fs::write(legacy.join("settings.json"), "old settings").unwrap();
        fs::write(legacy.join("style.md"), "old style").unwrap();
        fs::write(legacy.join("diagnostics.log"), "old log").unwrap();
        let result = migrate_legacy_data(&legacy, &data);
        assert_eq!(result, Migration { copied: vec!["settings.json", "style.md"], failed: vec![] });
        assert_eq!(fs::read_to_string(data.join("style.md")).unwrap(), "old style");
        assert!(legacy.join("style.md").exists());
        assert!(!data.join("diagnostics.log").exists());
    }

    #[test]
    fn migration_never_overwrites() {
        let (_root, legacy, data) = dirs();
        fs::create_dir(&data).unwrap();
        fs::write(legacy.join("style.md"), "old style").unwrap();
        fs::write(data.join("style.md"), "new style").unwrap();
        assert_eq!(migrate_legacy_data(&legacy, &data), Migration::default());
        assert_eq!(fs::read_to_string(data.join("style.md")).unwrap(), "new style");
    }

    #[test]
    fn migration_does_nothing_without_a_legacy_folder() {
        let (_root, legacy, data) = dirs();
        assert_eq!(migrate_legacy_data(&legacy.join("missing"), &data), Migration::default());
    }

    #[test]
    fn migration_reports_a_failed_copy() {
        let (_root, legacy, data) = dirs();
        fs::write(&data, "a file where the data folder should be").unwrap();
        fs::write(legacy.join("style.md"), "old style").unwrap();
        let result = migrate_legacy_data(&legacy, &data);
        assert_eq!(result, Migration { copied: vec![], failed: vec!["style.md"] });
    }

    #[test]
    fn log_appends_json_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("diagnostics.log");
        log_event(&path, "started", &[("version", "1".into())]);
        log_event(&path, "bar-shown", &[]);
        let text = fs::read_to_string(&path).unwrap();
        let lines: Vec<serde_json::Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["event"], "started");
        assert_eq!(lines[0]["version"], "1");
    }
}
