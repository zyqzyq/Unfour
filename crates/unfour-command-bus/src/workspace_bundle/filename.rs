pub(super) fn suggested_file_name(name: &str) -> String {
    // Leave room for the format suffix within common filesystem component limits.
    const MAX_STEM_BYTES: usize = 180;
    let mut stem = String::new();
    for ch in name.trim().chars() {
        let separator = ch.is_control()
            || ch.is_whitespace()
            || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*');
        let ch = if separator {
            if stem.is_empty() || stem.ends_with('-') {
                continue;
            }
            '-'
        } else {
            ch
        };
        if stem.len() + ch.len_utf8() > MAX_STEM_BYTES {
            break;
        }
        stem.push(ch);
    }
    let stem = stem.trim_matches([' ', '.', '-']);
    // Windows device names remain reserved even when followed by an extension.
    let device = stem
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].into_iter().any(|prefix| {
            device.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
    let stem = if stem.is_empty() || reserved {
        "workspace"
    } else {
        stem
    };
    format!("{stem}.unfour-workspace.json")
}

#[cfg(test)]
mod tests {
    use super::suggested_file_name;

    #[test]
    fn bundle_file_name_preserves_unicode_and_removes_path_characters() {
        for (name, expected) in [
            ("My Workspace", "My-Workspace"),
            ("客户 / API: v2?", "客户-API-v2"),
            (" ../客户 \\ API: v2? * . ", "客户-API-v2"),
            ("a\n\t<>:\"/\\|?*b", "a-b"),
        ] {
            assert_eq!(
                suggested_file_name(name),
                format!("{expected}.unfour-workspace.json")
            );
        }
    }

    #[test]
    fn bundle_file_name_falls_back_for_empty_or_reserved_names() {
        for name in ["", " ... ", "///", "CON", "nul.backup", "LPT9", "com¹"] {
            assert_eq!(suggested_file_name(name), "workspace.unfour-workspace.json");
        }
    }

    #[test]
    fn bundle_file_name_bounds_unicode_without_truncating_the_format_suffix() {
        assert_eq!(
            suggested_file_name(&"工".repeat(100)),
            format!("{}.unfour-workspace.json", "工".repeat(60))
        );
    }
}
