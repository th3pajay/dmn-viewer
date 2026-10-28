// Embeds the icon and version information (shown in Explorer under Properties > Details) in the Windows executables.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/icon.ico")
        .set("ProductName", "DMN Viewer")
        .set("FileDescription", "DMN Viewer: view, edit, check, test and fix DMN decision tables")
        .set("CompanyName", "th3pajay")
        .set("LegalCopyright", "Copyright (c) 2026 th3pajay. MIT License.");
    if let Err(e) = res.compile() {
        panic!("could not embed the Windows resources: {e}");
    }
}
