//! Display-name allocation shared by import services. Identity remains ID-based.
use crate::{AppError, AppResult};

pub fn import_copy_name<'a>(
    base: &str,
    existing: impl IntoIterator<Item = &'a str>,
    max: usize,
) -> AppResult<String> {
    if base.chars().count() > max {
        return Err(AppError::Validation(format!(
            "name must be {max} characters or fewer"
        )));
    }
    let existing: Vec<_> = existing.into_iter().collect();
    let mut name = base.to_string();
    let mut index = 1;
    while existing
        .iter()
        .any(|value| value.eq_ignore_ascii_case(&name))
    {
        let suffix = format!(" (Copy {index})");
        let remaining = max.checked_sub(suffix.chars().count()).ok_or_else(|| {
            AppError::Validation(format!("name must be {max} characters or fewer"))
        })?;
        let stem: String = base.chars().take(remaining).collect();
        name = format!("{stem}{suffix}");
        index += 1;
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_copy_name_rejects_base_over_max_without_a_conflict() {
        let error = import_copy_name("abcdef", std::iter::empty(), 5).unwrap_err();
        assert!(matches!(error, AppError::Validation(message) if message.contains("5")));
    }

    #[test]
    fn import_copy_name_uses_ascii_case_insensitive_matches() {
        let name = import_copy_name("Portable", ["PORTABLE", "portable (copy 1)"], 120).unwrap();
        assert_eq!(name, "Portable (Copy 2)");
    }
}
