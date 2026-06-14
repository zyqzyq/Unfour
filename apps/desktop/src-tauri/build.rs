fn main() {
    tauri_build::build();

    #[cfg(target_os = "windows")]
    {
        let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        println!("cargo:rustc-link-search=native={}", out_dir.display());
    }
}
