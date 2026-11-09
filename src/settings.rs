use serde_json::{Map, Value, json};
use std::fs;
use std::path::Path;

pub const HOTKEYS: [&str; 3] = ["Control+Alt+R", "Control+Alt+D", "Control+Shift+Space"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    Sonnet,
    Haiku,
}

impl Model {
    pub const ALL: [Model; 2] = [Model::Sonnet, Model::Haiku];

    pub fn key(self) -> &'static str {
        match self {
            Model::Sonnet => "sonnet",
            Model::Haiku => "haiku",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Model::Sonnet => "Sonnet: better writing",
            Model::Haiku => "Haiku: faster",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.key() == key)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub hotkey: String,
    pub model: Model,
    pub cli_path: String,
    pub primary_language: String,
    pub secondary_language: String,
    pub auto_bar: bool,
    pub style_template_installed: bool,
    /// Where Add to vocabulary sends entries; empty hides the action.
    pub inbox_url: String,
}

pub fn default_cli_path(windows: bool, home: &str) -> String {
    if windows {
        format!("{}\\.local\\bin\\claude.exe", home.trim_end_matches('\\'))
    } else {
        format!("{}/.local/bin/claude", home.trim_end_matches('/'))
    }
}

pub fn defaults() -> Settings {
    let home = dirs::home_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    Settings {
        hotkey: HOTKEYS[0].to_string(),
        model: Model::Sonnet,
        cli_path: default_cli_path(cfg!(windows), &home),
        primary_language: "Vietnamese".into(),
        secondary_language: "English".into(),
        auto_bar: true,
        style_template_installed: false,
        inbox_url: String::new(),
    }
}

fn non_empty(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).filter(|s| !s.trim().is_empty()).map(str::to_string)
}

/// Keeps each valid value and uses the default for anything else.
pub fn parse_settings(raw: &Value, defaults: &Settings) -> Settings {
    let empty = Map::new();
    let input = raw.as_object().unwrap_or(&empty);
    let get = |key: &str| input.get(key);
    Settings {
        hotkey: get("hotkey")
            .and_then(Value::as_str)
            .filter(|h| HOTKEYS.contains(h))
            .map_or_else(|| defaults.hotkey.clone(), str::to_string),
        model: get("model").and_then(Value::as_str).and_then(Model::from_key).unwrap_or(defaults.model),
        cli_path: non_empty(get("cliPath")).unwrap_or_else(|| defaults.cli_path.clone()),
        primary_language: non_empty(get("primaryLanguage")).unwrap_or_else(|| defaults.primary_language.clone()),
        secondary_language: non_empty(get("secondaryLanguage")).unwrap_or_else(|| defaults.secondary_language.clone()),
        auto_bar: get("autoBar").and_then(Value::as_bool).unwrap_or(defaults.auto_bar),
        style_template_installed: get("styleTemplateInstalled")
            .and_then(Value::as_bool)
            .unwrap_or(defaults.style_template_installed),
        inbox_url: get("inboxUrl")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| is_inbox_url(url))
            .map_or_else(|| defaults.inbox_url.clone(), str::to_string),
    }
}

/// An empty value turns the action off; anything else must be an http(s) address.
pub fn is_inbox_url(url: &str) -> bool {
    url.is_empty() || url.starts_with("https://") || url.starts_with("http://")
}

pub fn load_settings(path: &Path, defaults: &Settings) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .map_or_else(|| defaults.clone(), |raw| parse_settings(&raw, defaults))
}

/// Writes a temporary file and renames it, so a crash never leaves half a file.
pub fn save_settings(path: &Path, settings: &Settings) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let value = json!({
        "hotkey": settings.hotkey,
        "model": settings.model.key(),
        "cliPath": settings.cli_path,
        "primaryLanguage": settings.primary_language,
        "secondaryLanguage": settings.secondary_language,
        "autoBar": settings.auto_bar,
        "styleTemplateInstalled": settings.style_template_installed,
        "inboxUrl": settings.inbox_url,
    });
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_string_pretty(&value)?)?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Settings {
        Settings {
            hotkey: "Control+Alt+R".into(),
            model: Model::Sonnet,
            cli_path: "/default/claude".into(),
            primary_language: "Vietnamese".into(),
            secondary_language: "English".into(),
            auto_bar: true,
            style_template_installed: false,
            inbox_url: String::new(),
        }
    }

    #[test]
    fn offers_the_hotkeys_from_the_spec() {
        assert_eq!(HOTKEYS, ["Control+Alt+R", "Control+Alt+D", "Control+Shift+Space"]);
    }

    #[test]
    fn keeps_valid_values() {
        let parsed = parse_settings(
            &json!({ "model": "haiku", "autoBar": false, "primaryLanguage": "Japanese", "hotkey": "Control+Alt+D" }),
            &defaults(),
        );
        assert_eq!(parsed.model, Model::Haiku);
        assert!(!parsed.auto_bar);
        assert_eq!(parsed.primary_language, "Japanese");
        assert_eq!(parsed.hotkey, "Control+Alt+D");
    }

    #[test]
    fn falls_back_to_defaults_for_invalid_values() {
        for raw in [
            json!({ "hotkey": "Control+Q" }),
            json!({ "model": "gpt" }),
            json!({ "cliPath": 5 }),
            json!({ "cliPath": "  " }),
            json!({ "primaryLanguage": "" }),
            json!({ "secondaryLanguage": null }),
            json!({ "autoBar": "yes" }),
            json!({ "inboxUrl": "ftp://x" }),
            json!({ "inboxUrl": 3 }),
            json!(null),
            json!("text"),
            json!([]),
        ] {
            assert_eq!(parse_settings(&raw, &defaults()), defaults(), "{raw}");
        }
    }

    #[test]
    fn returns_defaults_for_a_missing_or_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert_eq!(load_settings(&path, &defaults()), defaults());
        fs::write(&path, "{not json").unwrap();
        assert_eq!(load_settings(&path, &defaults()), defaults());
    }

    #[test]
    fn saves_and_loads_with_the_electron_key_names() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        save_settings(&path, &Settings { model: Model::Haiku, ..defaults() }).unwrap();
        assert_eq!(load_settings(&path, &defaults()).model, Model::Haiku);
        let raw: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["model"], "haiku");
        assert_eq!(raw["cliPath"], "/default/claude");
    }

    #[test]
    fn keeps_and_saves_the_inbox_url() {
        let url = "https://abc.mockapi.io/api/v1/inbox";
        assert_eq!(parse_settings(&json!({ "inboxUrl": format!(" {url} ") }), &defaults()).inbox_url, url);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        save_settings(&path, &Settings { inbox_url: url.into(), ..defaults() }).unwrap();
        assert_eq!(load_settings(&path, &defaults()).inbox_url, url);
    }

    #[test]
    fn uses_the_native_cli_location_on_each_platform() {
        assert_eq!(default_cli_path(false, "/Users/a"), "/Users/a/.local/bin/claude");
        assert_eq!(default_cli_path(true, "C:\\Users\\a"), "C:\\Users\\a\\.local\\bin\\claude.exe");
    }
}
