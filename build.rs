fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        winresource::WindowsResource::new()
            .set_icon_with_id("assets/icon.ico", "1")
            .compile()
            .expect("Failed to compile resource file");
    }
}
