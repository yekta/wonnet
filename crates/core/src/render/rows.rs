//! The words of a thread row: who wrote, as "Alice, me (3)".

use std::collections::HashSet;

use mail_protocol::Address;

/// The thread's senders in the order they first wrote, "me" for the user's own addresses, first
/// names when there are several, with the number of messages when there is more than one.
pub fn senders(from: &[Address], me: &HashSet<String>) -> String {
    let mut people: Vec<&Address> = Vec::new();
    for address in from {
        if !people.iter().any(|known| known.email == address.email) {
            people.push(address);
        }
    }
    let name = |address: &Address, short: bool| -> String {
        if me.contains(&address.email) {
            return "me".into();
        }
        let full = address.display();
        match short {
            true => full.split_whitespace().next().unwrap_or(full).to_string(),
            false => full.to_string(),
        }
    };
    let short = people.len() > 1;
    let names: Vec<String> = match people.len() {
        0 => vec!["(no sender)".into()],
        1..=3 => people.iter().map(|address| name(address, short)).collect(),
        count => vec![name(people[0], true), "…".into(), name(people[count - 2], true), name(people[count - 1], true)],
    };
    let mut text = names.join(", ").replace(", …,", " …");
    if from.len() > 1 {
        text.push_str(&format!(" ({})", from.len()));
    }
    text
}

/// The line under a thread's subject, as who wrote and who they wrote to. Mail one person sent
/// is "Roger Cotes" to "me@example.com"; a conversation, where several wrote, is everyone in it,
/// "Roger, Ann & me@example.com", with nothing after. The user's own addresses are written out,
/// last, so it shows which of their accounts the mail is in.
pub fn participants(from: &[Address], to: &[Address], me: &HashSet<String>) -> (String, String) {
    let distinct = |addresses: &[&Address]| -> Vec<Address> {
        let mut seen = HashSet::new();
        addresses
            .iter()
            .filter(|address| seen.insert(address.email.clone()))
            .map(|address| (*address).clone())
            .collect()
    };
    let names = |people: &[Address]| -> Vec<String> {
        let short = people.len() > 1;
        people
            .iter()
            .map(|address| {
                if me.contains(&address.email) {
                    return address.email.clone();
                }
                let full = address.display();
                match short {
                    true => full.split_whitespace().next().unwrap_or(full).to_string(),
                    false => full.to_string(),
                }
            })
            .collect()
    };
    let writers = distinct(&from.iter().collect::<Vec<_>>());
    if writers.len() > 1 {
        let (mine, others): (Vec<&Address>, Vec<&Address>) =
            from.iter().chain(to).partition(|address| me.contains(&address.email));
        let mut everyone = names(&distinct(&[others, mine].concat()));
        let last = everyone.pop().unwrap_or_default();
        return (format!("{} & {last}", everyone.join(", ")), String::new());
    }
    let recipients: Vec<&Address> =
        to.iter().filter(|address| !writers.iter().any(|writer| writer.email == address.email)).collect();
    (names(&writers).join(", "), names(&distinct(&recipients)).join(", "))
}

/// A snippet without the invisible words newsletters pad their preview with (`&#847;&zwnj;`
/// and the like), which macOS stretches a line's spaces over.
pub fn snippet(text: &str) -> String {
    let invisible = |c: char| {
        matches!(c, '\u{00AD}' | '\u{034F}' | '\u{115F}' | '\u{1160}' | '\u{17B4}' | '\u{17B5}' | '\u{180E}')
            || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}')
            || matches!(c, '\u{3164}' | '\u{FEFF}' | '\u{FFA0}')
    };
    text.split_whitespace().filter(|word| !word.chars().all(invisible)).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(name: &str, email: &str) -> Address {
        Address::new(Some(name), email)
    }

    fn me() -> HashSet<String> {
        HashSet::from(["me@example.com".to_string()])
    }

    #[test]
    fn one_sender_keeps_the_full_name() {
        assert_eq!(senders(&[person("Alice Smith", "a@x.com")], &me()), "Alice Smith");
        assert_eq!(
            senders(&[person("Alice Smith", "a@x.com"), person("Alice Smith", "a@x.com")], &me()),
            "Alice Smith (2)"
        );
    }

    #[test]
    fn several_senders_use_first_names_and_me() {
        let thread =
            [person("Alice Smith", "a@x.com"), Address::new(None, "me@example.com"), person("Alice Smith", "a@x.com")];
        assert_eq!(senders(&thread, &me()), "Alice, me (3)");
    }

    #[test]
    fn many_senders_keep_the_first_and_the_last_two() {
        let thread = [
            person("Ann Lee", "a@x.com"),
            person("Bob Ray", "b@x.com"),
            person("Cy Twombly", "c@x.com"),
            person("Di Fox", "d@x.com"),
            person("Ed Wood", "e@x.com"),
        ];
        assert_eq!(senders(&thread, &me()), "Ann … Di, Ed (5)");
    }

    #[test]
    fn mail_one_person_sent_is_from_them_to_its_recipients() {
        let roger = person("Roger Cotes", "r@x.com");
        let me_address = Address::new(None, "me@example.com");
        assert_eq!(
            participants(std::slice::from_ref(&roger), &[me_address.clone(), me_address.clone()], &me()),
            ("Roger Cotes".into(), "me@example.com".into())
        );
        assert_eq!(participants(&[person("Ann Lee", "a@x.com")], &[], &me()), ("Ann Lee".into(), String::new()));
    }

    #[test]
    fn a_conversation_is_everyone_in_it_with_me_last() {
        let roger = person("Roger Cotes", "r@x.com");
        let me_address = Address::new(None, "me@example.com");
        assert_eq!(
            participants(&[me_address, roger.clone()], &[roger, person("Ann Lee", "a@x.com")], &me()),
            ("Roger, Ann & me@example.com".into(), String::new())
        );
    }

    #[test]
    fn snippets_drop_invisible_padding() {
        let padded = "Log in to view card details \u{034F}\u{200C} \u{034F}\u{200C} \u{00AD} \u{FEFF}";
        assert_eq!(snippet(padded), "Log in to view card details");
        assert_eq!(
            snippet("A family 👨\u{200D}👩\u{200D}👧 and می\u{200C}خواهم"),
            "A family 👨\u{200D}👩\u{200D}👧 and می\u{200C}خواهم"
        );
    }
}
