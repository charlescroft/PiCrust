fn main() {
    // gpui's Windows backend loads the executable's icon resource with id 1
    // (`LoadImageW(module, MAKEINTRESOURCE(1), ...)`). Without an embedded
    // resource the window and taskbar show the generic application icon, which
    // is what made the Windows build look "icon-less".
    //
    // `winresource` is a Windows-host build dependency, so this block is only
    // compiled there; the release workflow builds on windows-latest.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "PiCrust");
        res.set("FileDescription", "PiCrust — AI coding workbench");
        res.set("LegalCopyright", "Apache-2.0");
        if let Err(err) = res.compile() {
            panic!("failed to compile Windows resources: {err}");
        }
    }

    println!("cargo:rerun-if-changed=assets/icon.ico");
}
