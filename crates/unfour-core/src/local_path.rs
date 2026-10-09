//! Side-effect-free preflight. Drivers must still handle races and OS access errors.
use std::{fs, path::Path};

/// Existing task definitions may use paths relative to the application's working directory.
pub fn check_runtime(path: &str, destination: bool) -> Result<(), &'static str> {
    if path.contains("{{") {
        return Err("pathTemplate");
    }
    let local = Path::new(path);
    if local.is_absolute() {
        return check(path, destination);
    }
    if (cfg!(windows) && local.has_root())
        || (!cfg!(windows) && (path.as_bytes().get(1) == Some(&b':') || path.starts_with('\\')))
    {
        return Err("pathInvalid");
    }
    let absolute = std::env::current_dir()
        .map_err(|_| "pathInvalid")?
        .join(local);
    check(absolute.to_str().ok_or("pathInvalid")?, destination)
}

pub fn check(path: &str, destination: bool) -> Result<(), &'static str> {
    if path.contains("{{") {
        return Err("pathTemplate");
    }
    if destination && path.ends_with(['/', '\\']) {
        return Err("pathInvalid");
    }
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err("pathInvalid");
    }
    match fs::metadata(path) {
        Ok(metadata) if destination => {
            if !metadata.is_file() {
                Err("pathInvalid")
            } else if metadata.permissions().readonly() {
                Err("pathUnreadable")
            } else {
                Ok(())
            }
        }
        Ok(metadata) if metadata.is_file() => fs::File::open(path)
            .map(|_| ())
            .map_err(|_| "pathUnreadable"),
        Ok(_) => Err("pathInvalid"),
        Err(error) if destination && error.kind() == std::io::ErrorKind::NotFound => {
            // A download may create directories; inspect the nearest existing ancestor.
            let parent = path
                .ancestors()
                .skip(1)
                .find(|p| p.exists())
                .ok_or("pathMissing")?;
            let metadata = fs::metadata(parent).map_err(|_| "pathUnreadable")?;
            if !metadata.is_dir() {
                Err("pathInvalid")
            } else if metadata.permissions().readonly() {
                Err("pathUnreadable")
            } else {
                Ok(())
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err("pathMissing"),
        Err(_) => Err("pathUnreadable"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_checks_and_destination_preflight_do_not_create_files() {
        let root = std::env::temp_dir().join(format!("unfour-path-check-{}", crate::id::new_id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("source.txt");
        fs::write(&source, b"disposable").unwrap();
        let destination = root.join("missing").join("download.txt");
        assert_eq!(check(source.to_str().unwrap(), false), Ok(()));
        assert_eq!(
            check(destination.to_str().unwrap(), false),
            Err("pathMissing")
        );
        assert_eq!(check(destination.to_str().unwrap(), true), Ok(()));
        assert!(!root.join("missing").exists());
        assert_eq!(check(root.to_str().unwrap(), true), Err("pathInvalid"));
        assert_eq!(check("relative.txt", false), Err("pathInvalid"));
        assert_eq!(check("{{local_file}}", false), Err("pathTemplate"));
        fs::remove_file(source).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
