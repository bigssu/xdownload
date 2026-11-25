// Embeds the icon, version details and application manifest into the Windows executable and
// stamps the build time.
fn main() {
    // Re-run on any source change so the stamped build time stays current.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=assets/xdownload2.ico");
    println!("cargo:rerun-if-changed=assets/xdownload2.manifest");
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    println!("cargo:rustc-env=XDOWNLOAD2_BUILD_TIME={secs}");
    let mut resource = winresource::WindowsResource::new();
    resource.set_icon("assets/xdownload2.ico");
    // Shown in Explorer's Properties > Details and in the SmartScreen / UAC dialogs.
    resource.set("ProductName", "Xdownload2");
    resource.set("FileDescription", "Xdownload2 - YouTube downloader and media converter");
    resource.set("OriginalFilename", "xdownload2.exe");
    resource.set("InternalName", "xdownload2");
    resource.set("LegalCopyright", "Copyright (c) 2026 bigssu");
    resource.set_manifest_file("assets/xdownload2.manifest");
    resource.compile().expect("embed Windows resources");
}
