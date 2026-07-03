use unfour_command_bus::{default_database_path, DEFAULT_SECRET_SERVICE};
use unfour_secret_store::SecretStore;

#[test]
fn default_database_path_matches_unfour_paths() {
    let expected = unfour_paths::resolve_unfour_paths()
        .expect("resolve paths")
        .database_path;

    assert_eq!(
        default_database_path().expect("default database path"),
        expected
    );
}

#[test]
fn default_secret_service_keeps_existing_keychain_ref_format() {
    let store = SecretStore::in_memory(DEFAULT_SECRET_SERVICE);

    assert_eq!(DEFAULT_SECRET_SERVICE, "unfour");
    assert_eq!(
        store.make_ref("workspace-a", "database-password", "record-1"),
        "unfour:workspace-a:database-password:record-1"
    );
}
