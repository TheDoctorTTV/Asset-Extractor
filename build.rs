fn main() {
    println!("cargo:rerun-if-changed=assets/windows-icon.rc");
    println!("cargo:rerun-if-changed=asset-extractor-icon.ico");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // llvm-rc runs in the resource file's directory. Use an absolute icon
        // path so native Windows and Linux cross-builds resolve the same file.
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
            .expect("Cargo did not provide the project directory");
        let icon_path = std::path::Path::new(&manifest_dir).join("asset-extractor-icon.ico");
        let icon_definition = format!(
            "ASSET_EXTRACTOR_ICON=\"{}\"",
            icon_path.to_string_lossy().replace('\\', "/")
        );
        embed_resource::compile("assets/windows-icon.rc", [icon_definition])
            .manifest_required()
            .expect("Could not embed the Windows application icon");
    }
}
