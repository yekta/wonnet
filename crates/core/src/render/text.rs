//! Plain text mail as HTML: escaped, with links, and with quoted text folded away.

use linkify::{LinkFinder, LinkKind};

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(character),
        }
    }
    out
}

pub(crate) fn linked(line: &str) -> String {
    let mut out = String::new();
    for span in LinkFinder::new().spans(line) {
        let text = escape(span.as_str());
        match span.kind() {
            Some(LinkKind::Url) => out.push_str(&format!("<a href=\"{text}\">{text}</a>")),
            Some(LinkKind::Email) => out.push_str(&format!("<a href=\"mailto:{text}\">{text}</a>")),
            _ => out.push_str(&text),
        }
    }
    out
}

fn is_quote(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

/// How "Alice wrote:" ends, in the languages mail is most often written in.
const WROTE: [&str; 16] = [
    "wrote:",
    "writes:",
    "schrieb:",
    "a écrit :",
    "a écrit:",
    "ha scritto:",
    "escribió:",
    "escreveu:",
    "schreef:",
    "skrev:",
    "kirjoitti:",
    "napisał(a):",
    "napisał:",
    "написал(а):",
    "написал:",
    "yazdı:",
];

/// "On Mar 3, Alice wrote:" before a quote belongs to it.
pub(crate) fn introduces_quote(line: &str) -> bool {
    let line = line.trim();
    WROTE.iter().any(|wrote| line.ends_with(wrote))
}

/// Outlook quotes without `>`: everything from its "Original Message" line or its "From:" and
/// "Sent:" lines on.
fn starts_outlook_quote(lines: &[&str], start: usize) -> bool {
    let line = lines[start].trim();
    let sent = || lines[start + 1..].iter().take(2).any(|next| next.trim_start().starts_with("Sent:"));
    line.trim_matches('-').trim().eq_ignore_ascii_case("Original Message") && line.starts_with("---")
        || line.starts_with("From:") && sent()
}

/// Where the quote folded from this line on ends, when one starts here.
fn quote_end(lines: &[&str], start: usize) -> Option<usize> {
    let written_before = lines[..start].iter().any(|line| !line.trim().is_empty());
    if written_before && starts_outlook_quote(lines, start) {
        return Some(lines.len());
    }
    let quote_follows =
        lines[start + 1..].iter().find(|next| !next.trim().is_empty()).is_some_and(|next| is_quote(next));
    if !is_quote(lines[start]) && !(introduces_quote(lines[start]) && quote_follows) {
        return None;
    }
    let mut end = start + 1;
    while end < lines.len() && (is_quote(lines[end]) || lines[end].trim().is_empty()) {
        end += 1;
    }
    Some(end)
}

pub fn to_html(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::from("<div class=\"plain\">");
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if let Some(end) = quote_end(&lines, index) {
            let start = index;
            index = end;
            let quoted: Vec<String> = lines[start..index]
                .iter()
                .map(|line| linked(line.trim_start().trim_start_matches('>').trim_start_matches(' ')))
                .collect();
            out.push_str("<details class=\"quote\"><summary>•••</summary><blockquote>");
            out.push_str(&quoted.join("\n"));
            out.push_str("</blockquote></details>");
            continue;
        }
        out.push_str(&linked(line));
        out.push('\n');
        index += 1;
    }
    out.push_str("</div>");
    out
}

/// How many characters each line `to_html` shows has; a folded quote is its short pill.
pub fn line_lengths(text: &str) -> Vec<u32> {
    let lines: Vec<&str> = text.lines().collect();
    let mut lengths = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if let Some(end) = quote_end(&lines, index) {
            lengths.push(3);
            index = end;
            continue;
        }
        lengths.push(lines[index].trim_end().chars().count() as u32);
        index += 1;
    }
    lengths
}

/// HTML as plain text, roughly: for quoting a message in a reply when it has no text part.
pub fn from_html(html: &str) -> String {
    let mut out = String::new();
    let mut tag = String::new();
    let mut in_tag = false;
    let mut skipping = false;
    for character in html.chars() {
        match (in_tag, character) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let name: String = tag
                    .trim_start_matches('/')
                    .chars()
                    .take_while(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase();
                if name == "style" || name == "script" || name == "head" {
                    skipping = !tag.starts_with('/');
                }
                if ["br", "p", "div", "tr", "li", "h1", "h2", "h3"].contains(&name.as_str()) && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            (true, _) => tag.push(character),
            (false, _) if !skipping => out.push(character),
            _ => {}
        }
    }
    let decoded = out
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&");
    let mut lines: Vec<&str> = decoded.lines().map(str::trim_end).collect();
    lines.dedup_by(|a, b| a.is_empty() && b.is_empty());
    lines.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_links() {
        let html = to_html("Look <here>: https://example.com/a?b=1&c=2 or mail ann@example.com");
        assert!(html.contains("&lt;here&gt;"));
        assert!(html.contains("<a href=\"https://example.com/a?b=1&amp;c=2\">"));
        assert!(html.contains("<a href=\"mailto:ann@example.com\">"));
    }

    #[test]
    fn folds_quoted_text_with_its_introduction() {
        let html = to_html("Sounds good.\n\nOn Mar 3, Ann wrote:\n> Lunch?\n> Friday\n\nBye");
        assert!(html.starts_with("<div class=\"plain\">Sounds good.\n\n<details"), "{html}");
        assert!(html.contains("<blockquote>On Mar 3, Ann wrote:\nLunch?\nFriday\n</blockquote>"), "{html}");
        assert!(html.ends_with("</details>Bye\n</div>"), "{html}");
    }

    #[test]
    fn folds_quotes_in_other_languages_and_outlooks() {
        let italian = to_html("Va bene.\n\nIl giorno 20 ottobre, Ann ha scritto:\n> Pranzo?");
        assert!(
            italian.contains("<blockquote>Il giorno 20 ottobre, Ann ha scritto:\nPranzo?</blockquote>"),
            "{italian}"
        );
        let outlook = to_html("Sure.\n\n-----Original Message-----\nFrom: Ann\nSent: Monday\n\nLunch?");
        assert!(outlook.starts_with("<div class=\"plain\">Sure.\n\n<details"), "{outlook}");
        assert!(outlook.ends_with("Lunch?</blockquote></details></div>"), "{outlook}");
        let headers = to_html("Sure.\nFrom: Ann\nSent: Monday\nLunch?");
        assert!(headers.contains("<blockquote>From: Ann\nSent: Monday\nLunch?</blockquote>"), "{headers}");
        assert_eq!(line_lengths("Sure.\n\nFrom: Ann\nSent: Monday\nLunch?"), [5, 0, 3]);
        assert!(!to_html("-----Original Message-----\nFrom: Ann").contains("<details"));
    }

    #[test]
    fn reads_html_as_text() {
        assert_eq!(
            from_html("<style>p{}</style><p>Hi&nbsp;Ann,</p><p>See you<br>soon &amp; well</p>"),
            "Hi Ann,\nSee you\nsoon & well"
        );
    }
}
