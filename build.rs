fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/xwiki.ico");

    #[cfg(windows)]
    embed_resource::compile("assets/app.rc", embed_resource::NONE)
        .manifest_optional()
        .expect("failed to embed Windows application icon");
}
