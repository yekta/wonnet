//! Finds out about new releases and replaces this copy with one: it looks now and then, offers
//! the update, downloads it with its progress, puts it in this copy's place and offers the
//! restart. On Windows the running exe is replaced; on Linux the AppImage it runs from.

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

use gpui::{AppContext as _, Context};

const RELEASES: &str = "https://github.com/yekta/wonnet/releases";
/// The key the release workflow signs each desktop file with (Ed25519, raw, base64).
const PUBLIC_KEY: &str = "DsoTexc3mCyPllyu7NFzvovmvTwG/cHTxFkAUexXEDQ=";

/// Where releases are found. A debug build can be pointed at another place, with another key,
/// to try an update: `WONNET_RELEASES` and `WONNET_UPDATE_KEY`.
fn releases() -> String {
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("WONNET_RELEASES") {
        return url;
    }
    RELEASES.to_string()
}

fn public_key() -> Vec<u8> {
    use base64::Engine as _;
    #[cfg(debug_assertions)]
    let key = std::env::var("WONNET_UPDATE_KEY").unwrap_or_else(|_| PUBLIC_KEY.to_string());
    #[cfg(not(debug_assertions))]
    let key = PUBLIC_KEY.to_string();
    base64::engine::general_purpose::STANDARD.decode(key).unwrap_or_default()
}

/// Whether `signature` is the release key's over `file`.
fn signed(file: &[u8], signature: &[u8], key: &[u8]) -> bool {
    ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, key).verify(file, signature).is_ok()
}
const CHECK_EVERY: Duration = Duration::from_secs(6 * 3600);

/// This build's version, from the root Cargo.toml.
pub const VERSION: &str = env!("WONNET_VERSION");

#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    /// The user asked and the answer isn't here yet.
    Checking,
    UpToDate,
    Available(String),
    Downloading(String, f32),
    Installing(String),
    /// Installed; the running app is still the old one.
    Ready(String),
    Failed(String),
}

pub struct Updater {
    pub state: UpdateState,
    /// The state the banner was closed in: it stays away until the state changes.
    pub closed: Option<UpdateState>,
}

enum Progress {
    Fraction(f32),
    Installing,
    Done(Result<(), String>),
}

/// The file a release carries for this system.
fn asset() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        return Some("Wonnet-windows-x86_64.zip");
    }
    match (cfg!(target_os = "linux"), std::env::consts::ARCH) {
        (true, "x86_64") => Some("Wonnet-linux-x86_64.AppImage"),
        (true, "aarch64") => Some("Wonnet-linux-aarch64.AppImage"),
        _ => None,
    }
}

/// What this copy replaces when it updates.
fn target() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        return std::env::current_exe().ok();
    }
    std::env::var_os("APPIMAGE").map(PathBuf::from)
}

impl Updater {
    pub fn new() -> Self {
        Self { state: UpdateState::Idle, closed: None }
    }

    /// Why this copy can't replace itself, if it can't.
    pub fn obstacle() -> Option<&'static str> {
        if asset().is_none() {
            return Some("This copy updates itself only on Windows and Linux.");
        }
        if target().is_none() {
            return Some("This copy isn't an AppImage, so it can't update itself.");
        }
        None
    }

    /// Looks for a new release now and every few hours. A copy that can't replace itself doesn't.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        if Self::obstacle().is_some() {
            return;
        }
        self.check(false, cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(CHECK_EVERY).await;
                if this.update(cx, |this, cx| this.check(false, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn set(&mut self, state: UpdateState, cx: &mut Context<Self>) {
        self.state = state;
        self.closed = None;
        cx.notify();
    }

    pub fn close_banner(&mut self, cx: &mut Context<Self>) {
        self.closed = Some(self.state.clone());
        cx.notify();
    }

    pub fn banner_shown(&self) -> bool {
        self.state != UpdateState::Idle && self.closed.as_ref() != Some(&self.state)
    }

    /// `asked` is for when the user wants to know: finding nothing new is then said too.
    pub fn check(&mut self, asked: bool, cx: &mut Context<Self>) {
        if matches!(
            self.state,
            UpdateState::Checking | UpdateState::Downloading(..) | UpdateState::Installing(_) | UpdateState::Ready(_)
        ) {
            return;
        }
        if asked && let Some(obstacle) = Self::obstacle() {
            return self.set(UpdateState::Failed(obstacle.into()), cx);
        }
        if asked {
            self.set(UpdateState::Checking, cx);
        }
        let found = cx.background_spawn(async move { latest() });
        cx.spawn(async move |this, cx| {
            let found = found.await;
            let _ = this.update(cx, |this, cx| this.found(found, asked, cx));
        })
        .detach();
    }

    fn found(&mut self, found: Option<String>, asked: bool, cx: &mut Context<Self>) {
        let Some(version) = found else {
            if asked {
                self.set(UpdateState::Failed("GitHub couldn't be reached.".into()), cx);
            }
            return;
        };
        if !is_older(VERSION, &version) {
            if !asked {
                return;
            }
            self.set(UpdateState::UpToDate, cx);
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(Duration::from_secs(4)).await;
                let _ = this.update(cx, |this, cx| {
                    if this.state == UpdateState::UpToDate {
                        this.set(UpdateState::Idle, cx);
                    }
                });
            })
            .detach();
            return;
        }
        self.set(UpdateState::Available(version), cx);
    }

    /// Looks again, after a failure.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        self.state = UpdateState::Idle;
        self.check(true, cx);
    }

    pub fn install(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Available(version) = self.state.clone() else { return };
        let (Some(asset), Some(target)) = (asset(), target()) else {
            let obstacle = Self::obstacle().unwrap_or("This copy can't update itself.");
            return self.set(UpdateState::Failed(obstacle.into()), cx);
        };
        self.set(UpdateState::Downloading(version.clone(), 0.), cx);
        let (sender, progress) = async_channel::unbounded();
        let url = format!("{}/download/v{version}/{asset}", releases());
        std::thread::spawn(move || {
            let result = download_and_replace(&url, &target, &sender);
            let _ = sender.send_blocking(Progress::Done(result));
        });
        cx.spawn(async move |this, cx| {
            while let Ok(step) = progress.recv().await {
                let version = version.clone();
                let state = match step {
                    Progress::Fraction(fraction) => UpdateState::Downloading(version, fraction),
                    Progress::Installing => UpdateState::Installing(version),
                    Progress::Done(Ok(())) => UpdateState::Ready(version),
                    Progress::Done(Err(error)) => UpdateState::Failed(error),
                };
                if this.update(cx, |this, cx| this.set(state, cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Starts the installed version and quits this one.
    /// Starts the installed version, which waits for this one to have quit, and quits.
    pub fn relaunch(&self, cx: &mut Context<Self>) {
        if let Some(target) = target() {
            let _ = std::process::Command::new(target).arg(AFTER_UPDATE).spawn();
        }
        cx.quit();
    }
}

fn client() -> Option<reqwest::blocking::Client> {
    mail_protocol::tls::install();
    reqwest::blocking::Client::builder().timeout(Duration::from_secs(600)).user_agent("Wonnet").build().ok()
}

/// The argument the installed version starts with, to wait for the old one to quit.
pub const AFTER_UPDATE: &str = "--after-update";

/// The newest release's version, when its file for this system and the file's signature are
/// already up.
fn latest() -> Option<String> {
    let client = client()?;
    let releases = releases();
    let response = client.head(format!("{releases}/latest")).send().ok()?;
    let tag = response.url().path_segments()?.next_back()?.to_string();
    let version = tag.strip_prefix('v')?.to_string();
    let asset = asset()?;
    for file in [asset.to_string(), format!("{asset}.sig")] {
        let uploaded = client.head(format!("{releases}/download/v{version}/{file}")).send().ok()?;
        if !uploaded.status().is_success() {
            return None;
        }
    }
    Some(version)
}

fn download_and_replace(
    url: &str,
    target: &std::path::Path,
    progress: &async_channel::Sender<Progress>,
) -> Result<(), String> {
    let failed = |error: &dyn std::fmt::Display| format!("The new version couldn't be downloaded: {error}");
    let client = client().ok_or_else(|| failed(&"no connection"))?;
    let mut response = client.get(url).send().map_err(|error| failed(&error))?;
    if !response.status().is_success() {
        return Err(format!("The new version couldn't be downloaded ({}).", response.status().as_u16()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut bytes = Vec::with_capacity(total as usize);
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let read = response.read(&mut chunk).map_err(|error| failed(&error))?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if total > 0 {
            let _ = progress.send_blocking(Progress::Fraction(bytes.len() as f32 / total as f32));
        }
    }
    let signature = client
        .get(format!("{url}.sig"))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.bytes())
        .map_err(|error| failed(&error))?;
    if !signed(&bytes, &signature, &public_key()) {
        return Err("The download isn't signed by Wonnet, so it wasn't installed.".into());
    }
    let _ = progress.send_blocking(Progress::Installing);
    replace(&bytes, target)
}

#[cfg(target_os = "windows")]
fn replace(archive: &[u8], _target: &std::path::Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive))
        .map_err(|_| "The download isn't a copy of the app.".to_string())?;
    let mut exe = zip.by_name("Wonnet.exe").map_err(|_| "The download isn't a copy of the app.".to_string())?;
    let mut bytes = Vec::new();
    exe.read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    let fresh = std::env::temp_dir().join(format!("Wonnet-{}.exe", std::process::id()));
    std::fs::write(&fresh, bytes).map_err(|error| format!("The download couldn't be saved: {error}"))?;
    let replaced = self_replace::self_replace(&fresh).map_err(|error| format!("The app couldn't be replaced: {error}"));
    let _ = std::fs::remove_file(&fresh);
    replaced
}

#[cfg(not(target_os = "windows"))]
fn replace(image: &[u8], target: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let fresh = target.with_extension("new");
    std::fs::write(&fresh, image).map_err(|error| format!("The download couldn't be saved: {error}"))?;
    std::fs::set_permissions(&fresh, std::fs::Permissions::from_mode(0o755)).map_err(|error| error.to_string())?;
    std::fs::rename(&fresh, target).map_err(|error| format!("The app couldn't be replaced: {error}"))
}

/// How a version is shown: `v0.1.188`.
pub fn label(version: &str) -> String {
    format!("v{version}")
}

/// Whether `version` comes before `other`, comparing their numbers from the left.
pub fn is_older(version: &str, other: &str) -> bool {
    if version.is_empty() || other.is_empty() {
        return false;
    }
    let numbers = |text: &str| text.split('.').map(|part| part.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (ours, theirs) = (numbers(version), numbers(other));
    for index in 0..ours.len().max(theirs.len()) {
        let (left, right) = (ours.get(index).copied().unwrap_or(0), theirs.get(index).copied().unwrap_or(0));
        if left != right {
            return left < right;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_only_what_the_release_key_signed() {
        use base64::Engine as _;
        let signature = base64::engine::general_purpose::STANDARD
            .decode("dIFgBuk5PaPfB0L1liYB35lyL74SlQxgJWbs4Fw9DckWZHAevpvyoWaZvPuyb+d2/8CxJzKKI9NycyVGvGDIBg==")
            .unwrap();
        let key = base64::engine::general_purpose::STANDARD.decode(PUBLIC_KEY).unwrap();
        assert!(signed(b"wonnet update test", &signature, &key));
        let mut changed = b"wonnet update test".to_vec();
        changed[0] ^= 1;
        assert!(!signed(&changed, &signature, &key));
        let mut forged = signature.clone();
        forged[10] ^= 1;
        assert!(!signed(b"wonnet update test", &forged, &key));
    }

    #[test]
    fn compares_versions() {
        assert!(is_older("0.1.9", "0.1.10"));
        assert!(!is_older("0.2.0", "0.1.99"));
        assert!(!is_older("1.0", "1.0.0"));
    }
}
