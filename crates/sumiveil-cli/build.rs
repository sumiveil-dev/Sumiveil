fn main() {
    println!("cargo:rerun-if-changed=../../assets/sumiveil.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        let icon = std::path::Path::new("../../assets/sumiveil.ico");
        if icon.exists() {
            res.set_icon(icon.to_str().unwrap());
        }
        res.set("FileDescription", "Sumiveil command line");
        res.set("ProductName", "Sumiveil");
        res.set("OriginalFilename", "sumiveil.exe");
        res.set("LegalCopyright", "Copyright (c) 2026 sumiveil-dev");
        if let Err(e) = res.compile() {
            println!("cargo:warning=resource compile failed: {e}");
        }
    }
}
