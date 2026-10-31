//! What the desktop systems do differently: where the data is, the `wonnet` links, one copy of
//! the app at a time, notifications, printing.

pub mod instance;
pub mod notify;
pub mod scheme;

use std::path::PathBuf;

/// Where the app keeps its data: the core's database, its settings, the files going out.
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(std::env::temp_dir).join("Wonnet")
}

/// Prints a page: the browser opens it and asks where to print. Runs off the window's thread.
pub fn print(html: &str) -> Result<(), String> {
    let page = html
        .replacen("content=\"default-src 'none';", "content=\"default-src 'none'; script-src 'nonce-print';", 1)
        .replacen("</body>", "<script nonce=\"print\">window.onload = () => window.print();</script></body>", 1);
    let path = std::env::temp_dir().join(format!("wonnet-print-{}.html", std::process::id()));
    std::fs::write(&path, page).map_err(|error| format!("The page couldn't be printed: {error}"))?;
    open(&path).map_err(|error| format!("The page couldn't be printed: {error}"))
}

/// The folder files going out with drafts are copied into until the core uploads them.
pub fn outgoing_dir() -> PathBuf {
    data_dir().join("Outgoing")
}

/// Forgets files staged over a week ago: their drafts were sent or set aside long since.
pub fn prune_outgoing() {
    std::thread::spawn(|| {
        let week = std::time::Duration::from_secs(7 * 24 * 3600);
        let Ok(entries) = std::fs::read_dir(outgoing_dir()) else { return };
        for entry in entries.flatten() {
            let old = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|at| at.elapsed().ok())
                .is_some_and(|age| age > week);
            if old && std::fs::remove_dir_all(entry.path()).is_err() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    });
}

/// The app's icon as a file, for the system's notifications; written the first time.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn icon_path() -> PathBuf {
    let path = data_dir().join("wonnet.png");
    if !path.exists() {
        let _ = std::fs::write(&path, include_bytes!("../../assets/icon/wonnet.png"));
    }
    path
}

/// Moves the window with the pointer, as a press on its title bar does. Windows tells the bar
/// from the window's controls by area; one over the whole bar would swallow the clicks on them.
#[cfg(target_os = "windows")]
pub fn drag_window(window: &gpui::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, HTCAPTION, PostMessageW, WM_NCLBUTTONDOWN};

    let Ok(handle) = HasWindowHandle::window_handle(window) else { return };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
    let hwnd = HWND(handle.hwnd.get() as *mut _);
    let mut point = POINT::default();
    unsafe {
        if GetCursorPos(&mut point).is_err() {
            return;
        }
        let _ = ReleaseCapture();
        let position = ((point.y as u32 & 0xffff) << 16) | (point.x as u32 & 0xffff);
        let _ = PostMessageW(Some(hwnd), WM_NCLBUTTONDOWN, WPARAM(HTCAPTION as usize), LPARAM(position as isize));
    }
}

/// Opens a file in the app the system has for it.
pub fn open_file(path: &str) -> Result<(), String> {
    open(path).map_err(|error| format!("{path} couldn't be opened: {error}"))
}

/// Opens a link or a file in the app the system has for it, without the AppImage's GIO modules,
/// which AppRun points the app's own GIO at.
pub fn open(target: impl AsRef<std::ffi::OsStr>) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::process::CommandExt as _;
        use std::process::Stdio;

        let mut failed = None;
        for mut command in open::commands(target) {
            command.env_remove("GIO_MODULE_DIR").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            match command.process_group(0).spawn() {
                Ok(mut child) => {
                    std::thread::spawn(move || child.wait());
                    return Ok(());
                }
                Err(error) => failed = Some(error),
            }
        }
        Err(failed.unwrap_or_else(|| std::io::Error::other("nothing can open it")))
    }
    #[cfg(not(target_os = "linux"))]
    open::that_detached(target)
}
