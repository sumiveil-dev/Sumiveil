fn main() {
    println!("cargo:rerun-if-changed=../../assets/sumiveil.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        let icon = std::path::Path::new("../../assets/sumiveil.ico");
        if icon.exists() {
            res.set_icon(icon.to_str().unwrap());
        }
        res.set("FileDescription", "Sumiveil");
        res.set("ProductName", "Sumiveil");
        res.set("OriginalFilename", "sumiveil-gui.exe");
        res.set("LegalCopyright", "Copyright (c) 2026 sumiveil-dev");
        // Per-Monitor V2 DPI 対応を宣言 (高 DPI でぼやけないように)
        res.set_manifest(
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency><dependentAssembly><assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*"/></dependentAssembly></dependency>
  <application xmlns="urn:schemas-microsoft-com:asm.v3"><windowsSettings>
    <dpiAware xmlns="http://schemas.microsoft.com/SMI/2005/WindowsSettings">true/pm</dpiAware>
    <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
    <longPathAware xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">true</longPathAware>
  </windowsSettings></application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1"><application>
    <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
  </application></compatibility>
</assembly>"#,
        );
        if let Err(e) = res.compile() {
            println!("cargo:warning=resource compile failed: {e}");
        }
    }
}
