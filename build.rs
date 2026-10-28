// Make sure the embedded UI folder exists so `cargo build` works before the UI is built.
fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/dist");
    std::fs::create_dir_all(&dir).expect("create ui/dist");
    println!("cargo:rerun-if-changed=ui/dist");
}
