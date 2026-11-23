fn main() {
    println!("cargo:rerun-if-changed=assets/rights77.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut windows_resource = winresource::WindowsResource::new();
        windows_resource
            .set_icon("assets/rights77.ico")
            .set("ProductName", "Rights77 — Права 1С 7.7")
            .set("FileDescription", "Просмотр пользователей и прав 1С 7.7");
        windows_resource
            .compile()
            .expect("Не удалось встроить значок приложения");
    }
}
