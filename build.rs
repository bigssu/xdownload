// Embeds the Xdownload2 icon into the Windows executable and stamps the build time.
fn main() {
    // Re-run on any source change so the stamped build time stays current.
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=assets/xdownload2.ico");
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    println!("cargo:rustc-env=XDOWNLOAD2_BUILD_TIME={secs}");
    winresource::WindowsResource::new().set_icon("assets/xdownload2.ico").compile().expect("embed icon resource");
}
