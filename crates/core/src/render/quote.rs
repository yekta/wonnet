//! The history a reply quotes, folded under "•••" as Gmail trims it. It is found by the marks
//! mail apps leave around a quote (the list Mailgun's Talon keeps), in sanitized HTML: well
//! formed, and with only these of the mail's classes and ids left.

use std::ops::Range;

use super::text;

/// The classes `sanitize` keeps, by tag: Gmail's, Yahoo's, Thunderbird's and ours.
pub const CLASSES: [(&str, &[&str]); 2] = [
    ("div", &["gmail_quote", "gmail_quote_container", "yahoo_quoted", "moz-cite-prefix", "quote"]),
    ("blockquote", &["gmail_quote"]),
];
/// The ids Outlook wraps a quote in.
const OUTLOOK_QUOTES: [&str; 2] = ["OLK_SRC_BODY_SECTION", "mail-editor-reference-message-container"];
/// The ids Outlook puts before a quote, with the quote after them to the end.
const OUTLOOK_HEADERS: [&str; 2] = ["appendonsend", "divRplyFwdMsg"];
/// The ids `sanitize` keeps.
pub const IDS: [&str; 4] = [OUTLOOK_QUOTES[0], OUTLOOK_QUOTES[1], OUTLOOK_HEADERS[0], OUTLOOK_HEADERS[1]];

const VOID: [&str; 14] =
    ["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];

struct Element<'a> {
    name: &'a str,
    /// What its start tag holds: the name and the attributes.
    tag: &'a str,
    start: usize,
    /// Where its children start and end.
    inner: Range<usize>,
    end: usize,
    parent: Option<usize>,
}

/// The quoted history at the end of the page, when it has one and something else is written.
pub fn find(html: &str) -> Option<Range<usize>> {
    let elements = elements(html);
    let written = |part: &str| !text::from_html(part).trim().is_empty();
    (0..elements.len()).find_map(|index| {
        let element = &elements[index];
        let end = match mark(element) {
            Some(Mark::Quote) => element.end,
            Some(Mark::Header) => element.parent.map_or(html.len(), |parent| elements[parent].inner.end),
            None if element.name == "blockquote" && introduction(&elements, index, html).is_some() => element.end,
            None => return None,
        };
        let start = introduction(&elements, index, html).unwrap_or(element.start);
        let quoted = text::from_html(&html[start..end]);
        let forwarded = quoted.contains("Forwarded message") || quoted.contains("Begin forwarded message");
        (!forwarded && written(&html[..start]) && !written(&html[end..])).then_some(start..end)
    })
}

/// The page with the quote in a fold.
pub fn fold(html: &str, quote: Range<usize>) -> String {
    format!(
        "{}<details class=\"quote\"><summary>•••</summary>{}</details>{}",
        &html[..quote.start],
        &html[quote.clone()],
        &html[quote.end..]
    )
}

enum Mark {
    /// The element holds the quote.
    Quote,
    /// The element comes before the quote, which goes on to the end of its parent.
    Header,
}

fn mark(element: &Element) -> Option<Mark> {
    let class = attribute(element.tag, "class").unwrap_or_default();
    let classed = |classes: &[&str]| class.split_whitespace().any(|name| classes.contains(&name));
    let id = attribute(element.tag, "id").unwrap_or_default();
    let style: String = attribute(element.tag, "style").unwrap_or_default().split_whitespace().collect();
    // Outlook on the desktop starts a quote with a "From:" box ruled on top.
    let outlook_header = element.name == "div" && style.to_ascii_lowercase().contains("border:none;border-top:solid");
    if classed(&["gmail_quote", "gmail_quote_container", "yahoo_quoted"])
        || OUTLOOK_QUOTES.contains(&id)
        || (element.name == "div" && classed(&["quote"]))
        || (element.name == "blockquote" && attribute(element.tag, "type") == Some("cite"))
    {
        return Some(Mark::Quote);
    }
    (OUTLOOK_HEADERS.contains(&id) || outlook_header || classed(&["moz-cite-prefix"])).then_some(Mark::Header)
}

/// Where "On Mar 3, Ann wrote:" starts, when that line is just before the quote.
fn introduction(elements: &[Element], index: usize, html: &str) -> Option<usize> {
    let element = &elements[index];
    let parent_start = element.parent.map_or(0, |parent| elements[parent].inner.start);
    let siblings: Vec<&Element> = elements[..index].iter().filter(|sibling| sibling.parent == element.parent).collect();
    let previous = siblings.iter().rev().find(|sibling| sibling.name != "br");
    let after_previous = previous.map_or(parent_start, |sibling| sibling.end);
    let introduces = |part: &str| {
        let text = text::from_html(part);
        let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
        lines.next().is_some_and(text::introduces_quote) && lines.next().is_none()
    };
    // The line in text of the parent's own, after the last sibling or a line break.
    let breaks = siblings.iter().filter(|sibling| sibling.name == "br" && sibling.start >= after_previous);
    let starts = std::iter::once(after_previous).chain(breaks.map(|line_break| line_break.end));
    if let Some(start) = starts.into_iter().find(|&start| introduces(&html[start..element.start])) {
        return Some(start);
    }
    let sibling = previous?;
    let alone = text::from_html(&html[after_previous..element.start]).trim().is_empty();
    (alone && introduces(&html[sibling.start..sibling.end])).then_some(sibling.start)
}

/// A sanitized page's elements in order, each with its parent.
fn elements(html: &str) -> Vec<Element<'_>> {
    let mut elements: Vec<Element> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    let mut at = 0;
    while let Some(offset) = html[at..].find('<') {
        let start = at + offset;
        let Some(end) = tag_end(html, start) else { break };
        let tag = &html[start + 1..end - 1];
        at = end;
        if let Some(closing) = tag.strip_prefix('/') {
            let name = closing.trim();
            while let Some(index) = open.pop() {
                elements[index].inner.end = start;
                elements[index].end = end;
                if elements[index].name.eq_ignore_ascii_case(name) {
                    break;
                }
            }
            continue;
        }
        let name = &tag[..tag.find(|c: char| c.is_whitespace() || c == '/').unwrap_or(tag.len())];
        let void = VOID.contains(&name.to_ascii_lowercase().as_str());
        let close = if void { end } else { html.len() };
        elements.push(Element { name, tag, start, inner: end..close, end: close, parent: open.last().copied() });
        if !void {
            open.push(elements.len() - 1);
        }
    }
    elements
}

/// Just past the `>` that ends the tag starting at `start`, skipping quoted attribute values.
fn tag_end(html: &str, start: usize) -> Option<usize> {
    let mut quote = None;
    for (offset, &byte) in html.as_bytes()[start + 1..].iter().enumerate() {
        match (quote, byte) {
            (None, b'"' | b'\'') => quote = Some(byte),
            (Some(open), _) if open == byte => quote = None,
            (None, b'>') => return Some(start + 2 + offset),
            _ => {}
        }
    }
    None
}

/// An attribute's value as the sanitizer writes it: `name="value"`.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    tag[at..].split('"').next()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folded(html: &str) -> Option<String> {
        find(html).map(|quote| html[quote].to_string())
    }

    #[test]
    fn folds_gmails_quote() {
        let html = "<div dir=\"ltr\">Sure, Friday.</div><br><div class=\"gmail_quote\"><div class=\"gmail_attr\">On Mon, Ann wrote:<br></div><blockquote class=\"gmail_quote\">Lunch?</blockquote></div>";
        assert_eq!(folded(html).as_deref(), Some(&html[html.find("<div class=\"gmail_quote\"").unwrap()..]));
        let page = fold(html, find(html).unwrap());
        assert!(
            page.starts_with("<div dir=\"ltr\">Sure, Friday.</div><br><details class=\"quote\"><summary>"),
            "{page}"
        );
        assert!(page.ends_with("</blockquote></div></details>"), "{page}");
    }

    #[test]
    fn folds_apples_quote_with_its_introduction() {
        let html = "<div>Sounds good.</div><div><br><div>On Oct 29, 2026, at 11:24, Ann &lt;ann@x.com&gt; wrote:<br></div><br><blockquote type=\"cite\"><div>Lunch?</div></blockquote></div>";
        let quote = folded(html).unwrap();
        assert!(quote.starts_with("<div>On Oct 29"), "{quote}");
        assert!(quote.ends_with("</blockquote>"), "{quote}");
    }

    #[test]
    fn folds_an_introduction_in_any_language() {
        let html = "Va bene.<br>Il giorno 20 ottobre, Ann ha scritto:<br><blockquote>Pranzo?</blockquote>";
        assert_eq!(
            folded(html).as_deref(),
            Some("Il giorno 20 ottobre, Ann ha scritto:<br><blockquote>Pranzo?</blockquote>")
        );
    }

    #[test]
    fn folds_outlooks_quote() {
        let html = "<div><p>Thanks!</p><div id=\"appendonsend\"></div><hr><div id=\"divRplyFwdMsg\"><b>From:</b> Ann</div><div>Lunch?</div></div>";
        assert_eq!(
            folded(html).as_deref(),
            Some(
                "<div id=\"appendonsend\"></div><hr><div id=\"divRplyFwdMsg\"><b>From:</b> Ann</div><div>Lunch?</div>"
            )
        );
        let desktop = "<div><p>Thanks!</p><div style=\"border:none;border-top:solid var(--c0) 1.0pt;padding:3.0pt 0in 0in 0in\"><p><b>From:</b> Ann</p></div><p>Lunch?</p></div>";
        assert!(folded(desktop).unwrap().ends_with("<p>Lunch?</p>"));
    }

    #[test]
    fn folds_what_we_sent() {
        let html = "<div dir=\"auto\"><p>Yes</p></div><div class=\"quote\" dir=\"auto\"><p>On Mar 3, Ann wrote:</p><blockquote type=\"cite\"><p>Lunch?</p></blockquote></div>";
        assert!(folded(html).unwrap().starts_with("<div class=\"quote\""));
    }

    #[test]
    fn leaves_replies_between_the_quoted_lines_open() {
        let html = "<div>See below.</div><blockquote type=\"cite\">Lunch?</blockquote><div>Yes!</div><blockquote type=\"cite\">Where?</blockquote><div>Here.</div>";
        assert_eq!(folded(html), None);
    }

    #[test]
    fn leaves_forwards_and_lone_quotes_open() {
        let forward = "<div>FYI</div><div class=\"gmail_quote\"><div class=\"gmail_attr\">---------- Forwarded message ---------<br>From: Ann</div><div>Lunch?</div></div>";
        assert_eq!(folded(forward), None);
        assert_eq!(folded("<div class=\"gmail_quote\"><blockquote>Lunch?</blockquote></div>"), None);
        assert_eq!(folded("<p>A quote I like:</p><blockquote>To be.</blockquote>"), None);
    }
}
