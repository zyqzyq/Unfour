use std::fs;
use std::io;
use std::path::PathBuf;

pub const DEFAULT_PRODUCT_DATA_DIR: &str = "Unfour";
pub const DEFAULT_DATABASE_FILE: &str = "unfour.sqlite";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnfourPaths {
    pub product_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub backups_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageDiagnostics {
    pub product_data_dir: PathBuf,
    pub database_path: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub database_exists: bool,
    pub current_exe: Option<PathBuf>,
    pub current_working_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathRoots {
    data_dir: PathBuf,
    config_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
}

impl PathRoots {
    fn new(data_dir: PathBuf, config_dir: Option<PathBuf>, cache_dir: Option<PathBuf>) -> Self {
        Self {
            data_dir,
            config_dir,
            cache_dir,
        }
    }
}

pub fn resolve_unfour_paths() -> io::Result<UnfourPaths> {
    resolve_with_roots(&default_roots()?)
}

pub fn initialize_unfour_storage() -> io::Result<UnfourPaths> {
    initialize_with_roots(&default_roots()?)
}

pub fn storage_diagnostics() -> io::Result<StorageDiagnostics> {
    diagnostics_with_roots(&default_roots()?)
}

pub fn default_database_path() -> io::Result<PathBuf> {
    Ok(resolve_unfour_paths()?.database_path)
}

fn default_roots() -> io::Result<PathRoots> {
    let data_dir = dirs::data_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "OS data directory is not available",
        )
    })?;
    Ok(PathRoots::new(
        data_dir,
        dirs::config_dir(),
        dirs::cache_dir(),
    ))
}

fn resolve_with_roots(roots: &PathRoots) -> io::Result<UnfourPaths> {
    let product_data_dir = roots.data_dir.join(DEFAULT_PRODUCT_DATA_DIR);
    let database_path = product_data_dir.join(DEFAULT_DATABASE_FILE);
    let config_dir = roots
        .config_dir
        .as_ref()
        .map(|dir| dir.join(DEFAULT_PRODUCT_DATA_DIR))
        .unwrap_or_else(|| product_data_dir.join("config"));
    let cache_dir = roots
        .cache_dir
        .as_ref()
        .map(|dir| dir.join(DEFAULT_PRODUCT_DATA_DIR))
        .unwrap_or_else(|| product_data_dir.join("cache"));
    let backups_dir = product_data_dir.join("backups");

    Ok(UnfourPaths {
        product_data_dir,
        database_path,
        config_dir,
        cache_dir,
        backups_dir,
    })
}

fn initialize_with_roots(roots: &PathRoots) -> io::Result<UnfourPaths> {
    let paths = resolve_with_roots(roots)?;

    fs::create_dir_all(&paths.product_data_dir)?;
    fs::create_dir_all(&paths.config_dir)?;
    fs::create_dir_all(&paths.cache_dir)?;
    fs::create_dir_all(&paths.backups_dir)?;

    Ok(paths)
}

fn diagnostics_with_roots(roots: &PathRoots) -> io::Result<StorageDiagnostics> {
    let paths = resolve_with_roots(roots)?;

    Ok(StorageDiagnostics {
        database_exists: paths.database_path.exists(),
        product_data_dir: paths.product_data_dir,
        database_path: paths.database_path,
        config_dir: paths.config_dir,
        cache_dir: paths.cache_dir,
        backups_dir: paths.backups_dir,
        current_exe: std::env::current_exe().ok(),
        current_working_dir: std::env::current_dir().ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn unique_test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "unfour-paths-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn assert_ends_with(path: &Path, parts: &[&str]) {
        let suffix = parts.iter().collect::<PathBuf>();
        assert!(
            path.ends_with(&suffix),
            "expected {} to end with {}",
            path.display(),
            suffix.display()
        );
    }

    #[test]
    fn resolve_default_database_path_uses_stable_product_data_dir() {
        let paths = resolve_unfour_paths().expect("resolve paths");

        assert_ends_with(
            &paths.database_path,
            &[DEFAULT_PRODUCT_DATA_DIR, DEFAULT_DATABASE_FILE],
        );
        assert_eq!(
            paths.product_data_dir.file_name().unwrap(),
            DEFAULT_PRODUCT_DATA_DIR
        );
    }

    #[test]
    fn initialize_creates_storage_directories_without_creating_database() {
        let root = unique_test_root("create-dirs");
        let roots = PathRoots::new(
            root.join("data"),
            Some(root.join("config")),
            Some(root.join("cache")),
        );

        let paths = initialize_with_roots(&roots).expect("initialize storage");

        assert!(paths.product_data_dir.is_dir());
        assert!(paths.config_dir.is_dir());
        assert!(paths.cache_dir.is_dir());
        assert!(paths.backups_dir.is_dir());
        assert!(!paths.database_path.exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn diagnostics_reports_storage_state_without_logs_dir() {
        let root = unique_test_root("diagnostics");
        let roots = PathRoots::new(
            root.join("data"),
            Some(root.join("config")),
            Some(root.join("cache")),
        );
        let diagnostics = diagnostics_with_roots(&roots).expect("diagnostics");

        let StorageDiagnostics {
            product_data_dir,
            database_path,
            config_dir,
            cache_dir,
            backups_dir,
            database_exists,
            current_exe,
            current_working_dir,
        } = diagnostics;

        assert_ends_with(
            &database_path,
            &[DEFAULT_PRODUCT_DATA_DIR, DEFAULT_DATABASE_FILE],
        );
        assert_eq!(
            product_data_dir,
            root.join("data").join(DEFAULT_PRODUCT_DATA_DIR)
        );
        assert_eq!(
            config_dir,
            root.join("config").join(DEFAULT_PRODUCT_DATA_DIR)
        );
        assert_eq!(cache_dir, root.join("cache").join(DEFAULT_PRODUCT_DATA_DIR));
        assert_eq!(backups_dir, product_data_dir.join("backups"));
        assert!(!database_exists);
        let _ = current_exe;
        let _ = current_working_dir;

        let _ = std::fs::remove_dir_all(root);
    }
}
