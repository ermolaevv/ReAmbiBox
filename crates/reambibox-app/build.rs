fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/app.ico");
        res.set("ProductName", "ReAmbiBox");
        res.set("FileDescription", "ReAmbiBox Ambient Backlight Service");
        res.set("LegalCopyright", "Copyright (C) 2026 ReAmbiBox Authors");
        let _ = res.compile();
    }
}
