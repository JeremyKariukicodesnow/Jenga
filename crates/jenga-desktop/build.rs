fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set("ProductName", "Jenga");
        resource.set("FileDescription", "Jenga schematic editor");
        resource.set("LegalCopyright", "Jenga contributors. GPL-3.0-or-later.");
        resource.set_icon("../../resources/branding/jenga.ico");
        resource
            .compile()
            .expect("compile Windows version resources");
    }
}
