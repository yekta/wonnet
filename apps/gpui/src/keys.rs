//! What the keys can do, with the keys they have until the user changes them (Superhuman's), and
//! how keys are written: `e`, `shift+e`, `mod+k` (Ctrl+K here, ⌘K on a Mac), `g i` (G, then I).
//! A sign is the key it is on, on a US keyboard: `shift+3` is #. Ctrl is the Mac's ⌘ on Windows
//! and Linux, so `ctrl+1` and `mod+1` are the same key there.

use gpui::Keystroke;

/// Where a command's keys work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyScope {
    Everywhere,
    List,
    Thread,
}

impl KeyScope {
    pub fn overlaps(self, other: KeyScope) -> bool {
        self == KeyScope::Everywhere || other == KeyScope::Everywhere || self == other
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyCommand {
    pub id: String,
    pub title: String,
    pub group: &'static str,
    pub scope: KeyScope,
    pub defaults: Vec<String>,
}

impl KeyCommand {
    pub fn preference_key(&self) -> String {
        format!("keys:{}", self.id)
    }
}

pub const GROUPS: [&str; 6] = ["Moving around", "Selecting", "Conversations", "Writing", "Going to", "Wonnet"];

fn command(id: &str, title: &str, group: &'static str, scope: KeyScope, defaults: &[&str]) -> KeyCommand {
    KeyCommand {
        id: id.into(),
        title: title.into(),
        group,
        scope,
        defaults: defaults.iter().map(|key| key.to_string()).collect(),
    }
}

/// Every command the keys can run.
pub fn all() -> Vec<KeyCommand> {
    use KeyScope::*;
    let (moving, selecting, threads, writing, going) = (GROUPS[0], GROUPS[1], GROUPS[2], GROUPS[3], GROUPS[4]);
    let mut all = vec![
        command("next", "Next conversation", moving, Everywhere, &["j", "arrowdown"]),
        command("previous", "Previous conversation", moving, Everywhere, &["k", "arrowup"]),
        command("open", "Open", moving, List, &["enter", "o"]),
        command("top", "Jump to the top", moving, List, &["mod+arrowup"]),
        command("bottom", "Jump to the bottom", moving, List, &["mod+arrowdown"]),
        command("nextMessage", "Next message", moving, Thread, &["n"]),
        command("previousMessage", "Previous message", moving, Thread, &["p"]),
        command("expandAll", "Expand or collapse all messages", moving, Thread, &["shift+o"]),
        command("nextSplit", "Next split", moving, List, &["tab"]),
        command("previousSplit", "Previous split", moving, List, &["shift+tab"]),
        command("search", "Search", moving, Everywhere, &["/"]),
        command("palette", "Command palette", moving, Everywhere, &["mod+k"]),
        command("select", "Select", selecting, List, &["x"]),
        command("selectNext", "Select the next too", selecting, List, &["shift+j", "shift+arrowdown"]),
        command("selectPrevious", "Select the previous too", selecting, List, &["shift+k", "shift+arrowup"]),
        command("selectAll", "Select all", selecting, List, &["mod+a"]),
        command("archive", "Archive", threads, Everywhere, &["e"]),
        command("inbox", "Move to Inbox", threads, Everywhere, &["shift+e"]),
        command("snooze", "Snooze or remind me", threads, Everywhere, &["h"]),
        command("star", "Star", threads, Everywhere, &["s"]),
        command("read", "Mark read or unread", threads, Everywhere, &["u"]),
        command("markRead", "Mark read", threads, Everywhere, &["shift+i"]),
        command("trash", "Trash", threads, Everywhere, &["shift+3", "backspace"]),
        command("spam", "Spam", threads, Everywhere, &["shift+1"]),
        command("mute", "Mute", threads, Everywhere, &["shift+m"]),
        command("label", "Label", threads, Everywhere, &["l"]),
        command("removeLabel", "Remove this label", threads, Everywhere, &["y"]),
        command("move", "Move to", threads, Everywhere, &["v"]),
        command("unsubscribe", "Unsubscribe", threads, Everywhere, &["mod+u"]),
        command("undo", "Undo", threads, Everywhere, &["z", "mod+z"]),
        command("print", "Print", threads, Everywhere, &["mod+p"]),
        command("compose", "New message", writing, Everywhere, &["c", "mod+n"]),
        command("reply", "Reply", writing, Everywhere, &["r"]),
        command("replyAll", "Reply all", writing, Everywhere, &["a"]),
        command("replyAllInThread", "Reply all in an open thread", writing, Thread, &["enter"]),
        command("forward", "Forward", writing, Everywhere, &["f"]),
        command("go.inbox", "Inbox", going, Everywhere, &["g i"]),
        command("go.starred", "Starred", going, Everywhere, &["g s"]),
        command("go.sent", "Sent", going, Everywhere, &["g t"]),
        command("go.drafts", "Drafts", going, Everywhere, &["g d"]),
        command("go.archive", "Archive", going, Everywhere, &["g e"]),
        command("go.snoozed", "Snoozed", going, Everywhere, &["g h"]),
        command("go.spam", "Spam", going, Everywhere, &["g shift+1"]),
        command("go.trash", "Trash", going, Everywhere, &["g shift+3"]),
        command("go.label", "A label", going, Everywhere, &["g l"]),
        command("unreadOnly", "Unread only", going, List, &["shift+u"]),
        command("starredOnly", "Starred only", going, List, &["shift+s"]),
        command("go.account.0", "All inboxes", going, Everywhere, &["ctrl+0"]),
    ];
    for number in 1..=9 {
        all.push(KeyCommand {
            id: format!("go.account.{number}"),
            title: format!("Account {number}'s inbox"),
            group: going,
            scope: Everywhere,
            defaults: vec![format!("ctrl+{number}")],
        });
    }
    all.push(command("zero", "Get Me To Zero", GROUPS[5], List, &[]));
    all.push(command("settings", "Settings", GROUPS[5], Everywhere, &["mod+,"]));
    all.push(command("keybindings", "Keybindings", GROUPS[5], Everywhere, &["shift+/", "mod+/"]));
    all
}

const MAC: bool = cfg!(target_os = "macos");

/// What Shift types on each sign's key, on a US keyboard.
const SHIFTED: [(&str, &str); 21] = [
    ("1", "!"),
    ("2", "@"),
    ("3", "#"),
    ("4", "$"),
    ("5", "%"),
    ("6", "^"),
    ("7", "&"),
    ("8", "*"),
    ("9", "("),
    ("0", ")"),
    ("-", "_"),
    ("=", "+"),
    ("[", "{"),
    ("]", "}"),
    ("\\", "|"),
    (";", ":"),
    ("'", "\""),
    (",", "<"),
    (".", ">"),
    ("/", "?"),
    ("`", "~"),
];

/// A key as this system takes it: Ctrl is ⌘ off the Mac.
pub fn normalized(key: &str) -> String {
    if MAC {
        return key.to_string();
    }
    key.split(' ')
        .map(|stroke| {
            let mut parts: Vec<&str> =
                stroke.split('+').map(|part| if part == "ctrl" { "mod" } else { part }).collect();
            let name = parts.pop().unwrap_or_default();
            parts.dedup();
            let mut held: Vec<&str> = ["mod", "alt", "shift"].into_iter().filter(|held| parts.contains(held)).collect();
            held.push(name);
            held.join("+")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The key pressed with its modifiers, as `mod+shift+k`. None for a lone modifier.
pub fn stroke(keystroke: &Keystroke) -> Option<String> {
    let modifiers = &keystroke.modifiers;
    let mut name = match keystroke.key.as_str() {
        "up" => "arrowup".to_string(),
        "down" => "arrowdown".to_string(),
        "left" => "arrowleft".to_string(),
        "right" => "arrowright".to_string(),
        "" | "shift" | "control" | "alt" | "platform" | "function" | "capslock" => return None,
        key => key.to_lowercase(),
    };
    let mut shift = modifiers.shift;
    if let Some((base, _)) = SHIFTED.iter().find(|(_, sign)| *sign == name) {
        name = base.to_string();
        shift = true;
    }
    let mut held = Vec::new();
    if MAC {
        if modifiers.platform {
            held.push("mod");
        }
        if modifiers.control {
            held.push("ctrl");
        }
    } else {
        if modifiers.platform {
            return None;
        }
        if modifiers.control {
            held.push("mod");
        }
    }
    if modifiers.alt {
        held.push("alt");
    }
    if shift {
        held.push("shift");
    }
    held.push(&name);
    Some(held.join("+"))
}

/// The strokes of a key, one cap each: `g shift+3` reads G, #.
pub fn caps(key: &str) -> Vec<String> {
    normalized(key).split(' ').map(cap).collect()
}

/// The key as a line of text says it: "Ctrl+K", "G I".
pub fn label(key: &str) -> String {
    caps(key).join(" ")
}

fn cap(stroke: &str) -> String {
    let mut parts: Vec<&str> = stroke.split('+').collect();
    let name = parts.pop().unwrap_or_default();
    if parts == ["shift"]
        && let Some((_, sign)) = SHIFTED.iter().find(|(base, _)| *base == name)
    {
        return sign.to_string();
    }
    let named = match name {
        "enter" => {
            if MAC {
                "↩"
            } else {
                "Enter"
            }
        }
        "tab" => {
            if MAC {
                "⇥"
            } else {
                "Tab"
            }
        }
        "space" => "Space",
        "backspace" => {
            if MAC {
                "⌫"
            } else {
                "Backspace"
            }
        }
        "delete" => {
            if MAC {
                "⌦"
            } else {
                "Delete"
            }
        }
        "escape" => "Esc",
        "arrowup" => "↑",
        "arrowdown" => "↓",
        "arrowleft" => "←",
        "arrowright" => "→",
        "home" => "Home",
        "end" => "End",
        "pageup" => "Page Up",
        "pagedown" => "Page Down",
        _ => "",
    };
    let name = if named.is_empty() { name.to_uppercase() } else { named.to_string() };
    if MAC {
        let symbols: String = [("ctrl", "⌃"), ("alt", "⌥"), ("shift", "⇧"), ("mod", "⌘")]
            .iter()
            .filter(|(held, _)| parts.contains(held))
            .map(|(_, symbol)| *symbol)
            .collect();
        return symbols + &name;
    }
    let mut words: Vec<&str> = [("mod", "Ctrl"), ("alt", "Alt"), ("shift", "Shift")]
        .iter()
        .filter(|(held, _)| parts.contains(held))
        .map(|(_, word)| *word)
        .collect();
    words.push(&name);
    words.join("+")
}

/// Whether two keys can't both be kept: the same, or one the start of the other.
pub fn clash(one: &str, other: &str) -> bool {
    let (one, other) = (normalized(one), normalized(other));
    one == other || other.starts_with(&format!("{one} ")) || one.starts_with(&format!("{other} "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_keys_as_they_are_pressed() {
        assert_eq!(caps("g shift+3"), vec!["G", "#"]);
        assert!(clash("g", "g i"));
        assert!(!clash("g i", "g s"));
        let stroke = stroke(&Keystroke::parse("shift-3").unwrap()).unwrap();
        assert_eq!(stroke, "shift+3");
    }
}
