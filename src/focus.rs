//! Which app holds the selection, so focus can go back to it on macOS.

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn parse_bundle_id(lsappinfo_output: &str) -> Option<String> {
    let key = "\"CFBundleIdentifier\"=\"";
    let start = lsappinfo_output.find(key)? + key.len();
    let rest = &lsappinfo_output[start..];
    let end = rest.find('"')?;
    (end > 0).then(|| rest[..end].to_string())
}

/// True only when this app took focus from the app holding the selection.
pub fn should_refocus(source: Option<&str>, current: Option<&str>, own: &str) -> bool {
    matches!(source, Some(source) if source != own) && current == Some(own)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_bundle_id_from_lsappinfo() {
        assert_eq!(
            parse_bundle_id("\"CFBundleIdentifier\"=\"com.tinyspeck.slackmacgap\"\n").as_deref(),
            Some("com.tinyspeck.slackmacgap")
        );
        for output in ["", "kCFBundleIdentifier=NULL", "\"CFBundleIdentifier\"=[ NULL ]"] {
            assert_eq!(parse_bundle_id(output), None, "{output}");
        }
    }

    const OWN: &str = "local.fluent.app";

    #[test]
    fn refocuses_only_when_this_app_took_focus_from_another() {
        assert!(should_refocus(Some("com.google.Chrome"), Some(OWN), OWN));
        assert!(!should_refocus(Some("com.google.Chrome"), Some("com.google.Chrome"), OWN));
        assert!(!should_refocus(None, Some(OWN), OWN));
        assert!(!should_refocus(Some(OWN), Some(OWN), OWN));
        assert!(!should_refocus(Some("com.google.Chrome"), Some("com.apple.TextEdit"), OWN));
    }
}
