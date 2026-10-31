//! New mail as the system's notifications, from its sender, with their picture, while the
//! window is in the background. A click opens the thread; the buttons archive it, mark it read,
//! or open a reply. A thread's notifications are taken down once it is read or leaves the inbox,
//! here or elsewhere, and all of them at sign-out.
//!
//! On Linux the buttons answer on the notification's own thread. Windows starts the app with a
//! `wonnet://notification` link for them, which a running window is handed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::models::NewMailItem;

/// What a notification was clicked for.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Open(String),
    Archive(String),
    Read(String),
    Reply(String),
}

impl Answer {
    /// Reads a `wonnet://notification?do=…&thread=…` link.
    pub fn from_link(link: &str) -> Option<Self> {
        let query = link.strip_prefix("wonnet://notification?")?;
        let mut asked = None;
        let mut thread = None;
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=')?;
            match key {
                "do" => asked = Some(value.to_string()),
                "thread" => thread = Some(decode(value)?),
                _ => {}
            }
        }
        let thread = thread?;
        Some(match asked?.as_str() {
            "open" => Self::Open(thread),
            "archive" => Self::Archive(thread),
            "read" => Self::Read(thread),
            "reply" => Self::Reply(thread),
            _ => return None,
        })
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    fn link(asked: &str, thread: &str) -> String {
        format!("wonnet://notification?do={asked}&thread={}", encode(thread))
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn from_action(action: &str, thread: String) -> Option<Self> {
        Some(match action {
            "default" => Self::Open(thread),
            "archive" => Self::Archive(thread),
            "read" => Self::Read(thread),
            "reply" => Self::Reply(thread),
            _ => return None,
        })
    }
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The notifications shown, by thread; clones share them.
#[derive(Clone)]
pub struct Notifier {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    answers: async_channel::Sender<Answer>,
    /// Linux's ids of each thread's notifications.
    shown: Arc<Mutex<HashMap<String, Vec<u32>>>>,
}

impl Notifier {
    pub fn new(answers: async_channel::Sender<Answer>) -> Self {
        Self { answers, shown: Arc::default() }
    }

    /// The threads with a notification up.
    pub fn threads(&self) -> Vec<String> {
        self.shown.lock().map(|shown| shown.keys().cloned().collect()).unwrap_or_default()
    }

    /// Shows a message's notification, with its sender's picture when there is one.
    pub fn show(&self, message: NewMailItem, photo: Option<PathBuf>) {
        let notifier = self.clone();
        std::thread::spawn(move || notifier.show_now(message, photo));
    }

    /// Takes down the notifications of these threads.
    pub fn take_down(&self, threads: &[String]) {
        let Ok(mut shown) = self.shown.lock() else { return };
        let gone: Vec<(String, Vec<u32>)> = threads.iter().filter_map(|thread| shown.remove_entry(thread)).collect();
        drop(shown);
        if gone.is_empty() {
            return;
        }
        std::thread::spawn(move || {
            for (thread, ids) in gone {
                close(&thread, &ids);
            }
        });
    }

    pub fn take_down_all(&self) {
        let threads = self.threads();
        self.take_down(&threads);
        #[cfg(target_os = "windows")]
        std::thread::spawn(windows::clear);
    }

    #[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
    fn noted(&self, thread: &str, id: u32) {
        if let Ok(mut shown) = self.shown.lock() {
            shown.entry(thread.to_string()).or_default().push(id);
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn forget(&self, thread: &str, id: u32) {
        let Ok(mut shown) = self.shown.lock() else { return };
        let Some(ids) = shown.get_mut(thread) else { return };
        ids.retain(|kept| *kept != id);
        if ids.is_empty() {
            shown.remove(thread);
        }
    }
}

#[cfg_attr(not(any(target_os = "linux", target_os = "windows")), allow(dead_code))]
fn text(message: &NewMailItem) -> String {
    if message.snippet.is_empty() {
        message.subject.clone()
    } else {
        format!("{}\n{}", message.subject, message.snippet)
    }
}

#[cfg(target_os = "linux")]
impl Notifier {
    fn show_now(&self, message: NewMailItem, photo: Option<PathBuf>) {
        let mut notification = notify_rust::Notification::new();
        notification
            .appname("Wonnet")
            .summary(&message.from)
            .body(&text(&message))
            .hint(notify_rust::Hint::Category("email.arrived".into()))
            .action("default", "Open")
            .action("archive", "Archive")
            .action("read", "Mark as Read")
            .action("reply", "Reply");
        match photo.as_ref().and_then(|photo| photo.to_str()) {
            Some(photo) => notification.image_path(photo),
            None => notification.icon(&super::icon_path().display().to_string()),
        };
        let Ok(handle) = notification.show() else { return };
        let id = handle.id();
        self.noted(&message.thread, id);
        let answers = self.answers.clone();
        let thread = message.thread.clone();
        handle.wait_for_action(|action| {
            if let Some(answer) = Answer::from_action(action, thread.clone()) {
                let _ = answers.send_blocking(answer);
            }
        });
        self.forget(&message.thread, id);
    }
}

#[cfg(target_os = "linux")]
fn close(_thread: &str, ids: &[u32]) {
    let Ok(connection) = zbus::blocking::Connection::session() else { return };
    for id in ids {
        let _ = connection.call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "CloseNotification",
            &(*id,),
        );
    }
}

#[cfg(target_os = "windows")]
impl Notifier {
    fn show_now(&self, message: NewMailItem, photo: Option<PathBuf>) {
        if let Err(error) = windows::show(&message, photo) {
            eprintln!("The notification couldn't be shown: {error}");
            return;
        }
        self.noted(&message.thread, 0);
    }
}

#[cfg(target_os = "windows")]
fn close(thread: &str, _ids: &[u32]) {
    windows::remove(thread);
}

#[cfg(target_os = "windows")]
pub use windows::register;

#[cfg(target_os = "windows")]
mod windows {
    //! Toasts, which Windows shows only for an app it knows: a Start menu shortcut carries the
    //! app's id, which the process takes too.

    use std::path::PathBuf;

    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::Win32::Storage::EnhancedStorage::PKEY_AppUserModel_ID;
    use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, IPersistFile,
    };
    use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
    use windows::Win32::UI::Shell::{IShellLinkW, SetCurrentProcessExplicitAppUserModelID, ShellLink};
    use windows::core::{HSTRING, Interface, PCWSTR};

    use super::{Answer, text};
    use crate::models::NewMailItem;

    const APP_ID: &str = "app.wonnet";
    const GROUP: &str = "mail";

    /// Gives the process the app's id, and the Start menu its shortcut, off the window's thread.
    pub fn register() {
        unsafe {
            let _ = SetCurrentProcessExplicitAppUserModelID(&HSTRING::from(APP_ID));
        }
        std::thread::spawn(|| {
            if let Err(error) = shortcut() {
                eprintln!("The Start menu shortcut couldn't be made: {error}");
            }
        });
    }

    fn shortcut() -> windows::core::Result<()> {
        let Some(programs) = dirs::data_dir().map(|data| data.join("Microsoft\\Windows\\Start Menu\\Programs")) else {
            return Ok(());
        };
        let exe = std::env::current_exe()
            .map_err(|error| windows::core::Error::new(Default::default(), error.to_string()))?;
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
            link.SetPath(&HSTRING::from(exe.as_os_str()))?;
            link.SetDescription(&HSTRING::from("Wonnet"))?;
            let store: IPropertyStore = link.cast()?;
            store.SetValue(&PKEY_AppUserModel_ID, &PROPVARIANT::from(APP_ID))?;
            store.Commit()?;
            let file: IPersistFile = link.cast()?;
            file.Save(PCWSTR(HSTRING::from(programs.join("Wonnet.lnk").as_os_str()).as_ptr()), true)?;
        }
        Ok(())
    }

    /// A toast's tag: the thread, short enough for Windows.
    fn tag(thread: &str) -> String {
        thread.chars().filter(|c| c.is_ascii_alphanumeric()).take(60).collect()
    }

    fn escape(text: &str) -> String {
        text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    pub fn show(message: &NewMailItem, photo: Option<PathBuf>) -> windows::core::Result<()> {
        let link = |asked: &str| escape(&Answer::link(asked, &message.thread));
        let picture = photo
            .map(|photo| {
                format!(
                    "<image placement=\"appLogoOverride\" hint-crop=\"circle\" src=\"{}\"/>",
                    escape(&format!("file:///{}", photo.display().to_string().replace('\\', "/")))
                )
            })
            .unwrap_or_default();
        let lines: String =
            text(message).lines().map(|line| format!("<text>{}</text>", escape(line))).collect::<Vec<_>>().join("");
        let xml = format!(
            "<toast launch=\"{open}\" activationType=\"protocol\"><visual><binding template=\"ToastGeneric\"><text>{from}</text>{lines}{picture}</binding></visual><actions><action content=\"Archive\" activationType=\"protocol\" arguments=\"{archive}\"/><action content=\"Mark as Read\" activationType=\"protocol\" arguments=\"{read}\"/><action content=\"Reply\" activationType=\"protocol\" arguments=\"{reply}\"/></actions></toast>",
            open = link("open"),
            from = escape(&message.from),
            archive = link("archive"),
            read = link("read"),
            reply = link("reply"),
        );
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(xml))?;
        let toast = ToastNotification::CreateToastNotification(&document)?;
        toast.SetTag(&HSTRING::from(tag(&message.thread)))?;
        toast.SetGroup(&HSTRING::from(GROUP))?;
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?.Show(&toast)
    }

    pub fn remove(thread: &str) {
        let Ok(history) = ToastNotificationManager::History() else { return };
        let _ =
            history.RemoveGroupedTagWithId(&HSTRING::from(tag(thread)), &HSTRING::from(GROUP), &HSTRING::from(APP_ID));
    }

    pub fn clear() {
        let Ok(history) = ToastNotificationManager::History() else { return };
        let _ = history.ClearWithId(&HSTRING::from(APP_ID));
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl Notifier {
    fn show_now(&self, message: NewMailItem, photo: Option<PathBuf>) {
        let _ = (message, photo);
    }
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn close(_thread: &str, _ids: &[u32]) {}

#[cfg(test)]
mod tests {
    use super::Answer;

    #[test]
    fn reads_the_links_its_toasts_open() {
        let thread = "t:1/ä b";
        for answer in [
            Answer::Open(thread.into()),
            Answer::Archive(thread.into()),
            Answer::Read(thread.into()),
            Answer::Reply(thread.into()),
        ] {
            let asked = match &answer {
                Answer::Open(_) => "open",
                Answer::Archive(_) => "archive",
                Answer::Read(_) => "read",
                Answer::Reply(_) => "reply",
            };
            assert_eq!(Answer::from_link(&Answer::link(asked, thread)), Some(answer));
        }
        assert_eq!(Answer::from_link("wonnet://notification?do=delete&thread=x"), None);
        assert_eq!(Answer::from_link("wonnet://signed-in?code=x"), None);
    }
}
