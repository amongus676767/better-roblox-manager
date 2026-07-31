//! Embeds the Windows icon and version metadata into the executable.
//!
//! `eframe`'s `with_icon` only sets the icon of the *running window*. The icon
//! Explorer shows for `brm.exe`, and the one a pinned taskbar shortcut uses,
//! comes from a resource compiled into the binary — which is what this does.
//! Without it the exe has no icon at all and Windows falls back to a blank
//! default, which is why a pinned shortcut looked empty.
//!
//! Requires `rc.exe` from the Windows SDK, which ships with the same Visual
//! Studio Build Tools the MSVC linker needs, so anyone who can already build
//! this project has it.

fn main() {
    // Only meaningful on Windows, and `rc.exe` doesn't exist elsewhere.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rerun-if-changed=../assets/icon.ico");

    let mut res = winresource::WindowsResource::new();
    res.set_icon("../assets/icon.ico");
    res.set("ProductName", "Better Roblox Manager");
    res.set("FileDescription", "Better Roblox Manager");
    res.set("LegalCopyright", "MIT Licensed");

    // A missing resource compiler shouldn't break the build for someone who
    // just wants a working binary — warn and carry on without the icon.
    if let Err(e) = res.compile() {
        println!("cargo:warning=Could not embed the executable icon: {e}");
    }
}
