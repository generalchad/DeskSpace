fn main() {
    // Embed a manifest dependency on Common Controls v6. Without it Windows
    // loads the legacy comctl32 and the settings window's controls draw with
    // the Windows 95 look instead of the current visual style.
    // https://learn.microsoft.com/en-us/windows/win32/controls/cookbook-overview
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") && target.contains("msvc") {
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );

        // /Brepro zeroes the PE timestamp and replaces it with a content hash,
        // so two builds of the same sources differ less. It is one half of the
        // reproducible release build (task E3 in docs/archive/TASKS-done.md); the other is
        // --remap-path-prefix in release.yml.
        println!("cargo:rustc-link-arg-bins=/Brepro");

        // Embed the application icon and its version block. winresource fills
        // FileVersion, ProductVersion, and ProductName from the Cargo package;
        // the two fields below are the ones it cannot infer.
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon_with_id("../../assets/deskspace.ico", "1");
        resource.set("FileDescription", "DeskSpace");
        resource.set(
            "LegalCopyright",
            "Copyright 2026 Timothy W. Brown. Licensed under the Apache License 2.0.",
        );
        resource
            .compile()
            .expect("compile the Windows resource (needs the Windows SDK rc.exe)");
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/deskspace.ico");
}
