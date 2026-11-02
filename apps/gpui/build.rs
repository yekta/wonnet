//! Gives the app the version in the root Cargo.toml's `[workspace.package]`, which
//! `scripts/release-version.sh` writes for a release, on Windows its icon and version info, and
//! on Linux its WPE platform, built against the WPE WebKit of `scripts/build-wpe.sh`.

fn main() {
    let manifest = "../../Cargo.toml";
    println!("cargo:rerun-if-changed={manifest}");
    println!("cargo:rerun-if-changed=assets/icon/wonnet.ico");
    let text = std::fs::read_to_string(manifest).unwrap_or_default();
    let version = workspace_version(&text).unwrap_or_else(|| "0.0.0".into());
    println!("cargo:rustc-env=WONNET_VERSION={version}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        wpe();
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // GPUI lays out deep trees on the main thread; Windows gives it 1 MB unless asked.
        if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            println!("cargo:rustc-link-arg-bins=/STACK:8388608");
        }
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/icon/wonnet.ico")
            .set("ProductName", "Wonnet")
            .set("FileDescription", "Wonnet")
            .set("CompanyName", "Wonnet")
            .set("LegalCopyright", "MIT")
            .set("ProductVersion", &version)
            .set("FileVersion", &version);
        if let Err(error) = resource.compile() {
            println!("cargo:warning=The icon and version info weren't embedded: {error}");
        }
    }
}

/// Compiles `src/wpe/page.c` and links WPE WebKit from build/wpe, where the app finds it again
/// when it runs from the build. The AppImage points it at its own copy.
fn wpe() {
    println!("cargo:rerun-if-changed=src/wpe/page.c");
    println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
    let built =
        std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join("build/wpe/lib/pkgconfig");
    let search = std::env::var("PKG_CONFIG_PATH")
        .map_or_else(|_| built.display().to_string(), |path| format!("{}:{path}", built.display()));
    // SAFETY: the build script runs on one thread.
    unsafe { std::env::set_var("PKG_CONFIG_PATH", search) };
    let mut build = cc::Build::new();
    for package in ["wpe-webkit-2.0", "wpe-platform-2.0"] {
        let library = pkg_config::probe_library(package)
            .unwrap_or_else(|error| panic!("{package} wasn't found; build it with scripts/build-wpe.sh: {error}"));
        build.includes(&library.include_paths);
        for path in &library.link_paths {
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{}", path.display());
        }
    }
    build.file("src/wpe/page.c").warnings(false).compile("wonnet_wpe");
}

/// The `version` under `[workspace.package]`.
fn workspace_version(text: &str) -> Option<String> {
    let mut in_package = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_package = line == "[workspace.package]";
            continue;
        }
        if in_package && let Some(value) = line.strip_prefix("version") {
            return Some(value.trim_start().strip_prefix('=')?.trim().trim_matches('"').to_string());
        }
    }
    None
}
