fn main() {
    // The frontend is embedded from ../dist. Make sure the directory exists so the
    // crate also builds (e.g. `cargo test --workspace`) before `pnpm build` ran.
    std::fs::create_dir_all("../dist").expect("creating ../dist");
    tauri_build::build()
}
