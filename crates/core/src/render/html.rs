//! A message's body as the page a thread's card shows: sanitized, remote images blocked unless
//! asked for, inside the theme's CSS. Mail that brings its own design (tables, colours) is drawn
//! on a "paper" card, the colour of the thread's card; in the dark scheme its colours are turned
//! (`dark.rs`) unless the page is asked for the original, when the card is light as its sender
//! made it and padded from the thread's.

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use mail_protocol::Body;
use serde::Serialize;

use super::dark::Palette;
use super::text;

const TOKENS: &str = include_str!("../../../../packages/theme/tokens.css");

const STYLE: &str = r#"
html, body { margin: 0; padding: 0; background: transparent; }
body { font: 15px/1.55 "Avenir", -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif; color: var(--foreground);
  -webkit-text-size-adjust: 100%; overflow-wrap: break-word; box-sizing: border-box; }
a { color: var(--primary); }
img { max-width: 100%; height: auto; }
.plain { white-space: pre-wrap; }
blockquote { margin: 0 0 0 2px; padding-left: 12px; border-left: 2px solid var(--border-card); color: var(--muted-foreground); }
details.quote > summary { list-style: none; display: inline-block; cursor: pointer; padding: 0 8px; margin: 6px 0;
  border-radius: 9px; background: var(--background-secondary); color: var(--muted-foreground); font-size: 12px; line-height: 18px; letter-spacing: 1px; }
details.quote > summary::-webkit-details-marker { display: none; }
body.paper { background: var(--card-larger); color: var(--emphasized-foreground); color-scheme: light; border-radius: var(--radius-sm); }
@media (prefers-color-scheme: dark) {
  body.paper.dark { color: var(--foreground); color-scheme: dark; }
  body.paper.light { padding: 12px; }
}
table { max-width: 100%; }
"#;

pub struct Page {
    pub html: String,
    /// Remote images were left out; the client offers to load them.
    pub blocked_images: bool,
}

/// The page. With `dark`, the body carries the `dark` class: in the dark scheme the mail's
/// colours are turned. The class taken off shows the original.
pub fn page(body: &Body, show_images: bool, dark: bool) -> Page {
    let blocked = Arc::new(AtomicBool::new(false));
    let palette = Arc::new(Palette::default());
    let (content, paper) = match &body.html {
        Some(html) => (sanitize(html, show_images, blocked.clone(), palette.clone()), is_designed(html)),
        None => (text::to_html(body.text.as_deref().unwrap_or_default()), false),
    };
    let palette = palette.css();
    let images = if show_images { "data: cid: https: http:" } else { "data: cid:" };
    let class = match (paper, dark) {
        (true, true) => "paper dark",
        (true, false) => "paper light",
        (false, true) => "themed dark",
        (false, false) => "themed",
    };
    let html = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src {images}; style-src 'unsafe-inline'; font-src data:\">\
         <style>{TOKENS}{STYLE}{palette}</style></head><body class=\"{class}\">{content}</body></html>"
    );
    Page { html, blocked_images: blocked.load(Ordering::Relaxed) }
}

/// The most lines a shape has: past them the placeholder is off the screen.
const SHAPE_LINES: usize = 80;
/// A snippet shorter than this is likely the whole mail.
const WHOLE_SNIPPET: u32 = 120;
/// What most mail goes on with after its snippet.
const USUAL_REST: [u32; 6] = [0, 220, 160, 0, 90, 12];
/// Designed mail spends most of its height on padding and large type: it is never shorter.
const DESIGNED_LINES: usize = 16;
/// The tags that start a line of their own.
const BLOCKS: [&str; 15] =
    ["p", "div", "tr", "td", "li", "ul", "ol", "table", "blockquote", "h1", "h2", "h3", "h4", "h5", "h6"];

/// A line of a page's placeholder.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Line {
    /// Text of this many characters; none is a blank line.
    Text(u32),
    /// An image, as large as its attributes say.
    Image { width: u32, height: u32 },
}

/// The lines of the page's text and its images: the apps draw a placeholder about as tall as
/// the page from them while the page is drawn. A body not here yet is its snippet and, when the
/// snippet was cut, what most mail goes on with.
pub fn shape(body: Option<&Body>, snippet: &str) -> Vec<Line> {
    let mut lines = match body {
        Some(Body { html: Some(html), .. }) => {
            let mut lines = html_shape(html);
            if is_designed(html) && lines.len() < DESIGNED_LINES {
                lines.resize(DESIGNED_LINES, Line::Text(0));
            }
            lines
        }
        Some(Body { text, .. }) => {
            text::line_lengths(text.as_deref().unwrap_or_default()).into_iter().map(Line::Text).collect()
        }
        None => match snippet.chars().count() as u32 {
            length @ 1..WHOLE_SNIPPET => vec![Line::Text(length)],
            length => [length.max(WHOLE_SNIPPET)].into_iter().chain(USUAL_REST).map(Line::Text).collect(),
        },
    };
    lines.truncate(SHAPE_LINES);
    lines
}

fn html_shape(html: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    let mut length = 0;
    let mut tag = String::new();
    let (mut in_tag, mut skipping, mut spaced) = (false, false, true);
    for character in html.chars() {
        match (in_tag, character) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let closing = tag.starts_with('/');
                let name =
                    tag.trim_start_matches('/').split(|c: char| !c.is_ascii_alphanumeric()).next().unwrap_or_default();
                let name = name.to_ascii_lowercase();
                if ["style", "script", "head", "title"].contains(&name.as_str()) {
                    skipping = !closing;
                }
                let image = (name == "img").then(|| (attribute(&tag, "width"), attribute(&tag, "height")));
                let image = image.and_then(|(width, height)| Some(Line::Image { width: width?, height: height? }));
                let ends_line = name == "br" || image.is_some() || (BLOCKS.contains(&name.as_str()) && length > 0);
                if ends_line && (length > 0 || name == "br") {
                    lines.push(Line::Text(length));
                }
                if ends_line {
                    length = 0;
                    spaced = true;
                }
                if let Some(Line::Image { height: 8.., .. }) = image {
                    lines.extend(image);
                }
                if name == "p" && closing {
                    lines.push(Line::Text(0));
                }
            }
            (true, _) => tag.push(character),
            (false, _) if skipping => {}
            (false, space) if space.is_whitespace() => {
                if !spaced {
                    length += 1;
                }
                spaced = true;
            }
            (false, _) => {
                length += 1;
                spaced = false;
            }
        }
    }
    if length > 0 {
        lines.push(Line::Text(length));
    }
    while lines.last() == Some(&Line::Text(0)) {
        lines.pop();
    }
    lines
}

/// A number an HTML attribute gives, `height="300"`.
fn attribute(tag: &str, name: &str) -> Option<u32> {
    let tag = tag.to_ascii_lowercase();
    let mut rest = tag.as_str();
    while let Some(at) = rest.find(name) {
        let starts_word = rest[..at].ends_with(char::is_whitespace);
        rest = &rest[at + name.len()..];
        let Some(value) = rest.trim_start().strip_prefix('=').filter(|_| starts_word) else { continue };
        let digits: String =
            value.trim_start().trim_start_matches(['"', '\'']).chars().take_while(char::is_ascii_digit).collect();
        return digits.parse().ok();
    }
    None
}

/// Whether the mail sets its own look: then the theme's colours would fight it.
pub fn is_designed(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    lower.contains("bgcolor")
        || lower.contains("background")
        || lower.contains("<style")
        || (lower.contains("<table") && lower.matches("style=").count() > 3)
        || lower.matches("color:").count() > 5
}

fn is_remote(url: &str) -> bool {
    let url = url.trim_start().to_ascii_lowercase();
    url.starts_with("http:") || url.starts_with("https:") || url.starts_with("//")
}

/// Drops `url(...)` with remote addresses from inline CSS.
fn without_remote_urls(style: &str) -> String {
    let mut out = String::new();
    let mut rest = style;
    while let Some(start) = rest.to_ascii_lowercase().find("url(") {
        let end = rest[start..].find(')').map(|end| start + end + 1).unwrap_or(rest.len());
        let inner = rest[start + 4..end.saturating_sub(1).max(start + 4)].trim_matches(['\'', '"', ' ']);
        out.push_str(&rest[..start]);
        if !is_remote(inner) {
            out.push_str(&rest[start..end]);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The mail's colours go to `palette`, which makes the stylesheet the page needs for them.
pub fn sanitize(html: &str, show_images: bool, blocked: Arc<AtomicBool>, palette: Arc<Palette>) -> String {
    let mut builder = ammonia::Builder::default();
    builder
        .add_tags(["font", "tfoot", "caption"])
        .add_generic_attributes([
            "style",
            "align",
            "valign",
            "bgcolor",
            "width",
            "height",
            "border",
            "cellpadding",
            "cellspacing",
            "color",
            "dir",
            "background",
        ])
        .add_tag_attributes("font", ["face", "size"])
        .add_url_schemes(["data", "cid"])
        .link_rel(Some("noopener noreferrer"))
        .attribute_filter(move |element, attribute, value| match (element, attribute) {
            ("img", "src") | (_, "background") if !show_images && is_remote(value) => {
                blocked.store(true, Ordering::Relaxed);
                None
            }
            (_, "style") => {
                let mut style = Cow::Borrowed(value);
                if !show_images && value.to_ascii_lowercase().contains("url(") {
                    let cleaned = without_remote_urls(value);
                    if cleaned.len() != value.len() {
                        blocked.store(true, Ordering::Relaxed);
                    }
                    style = Cow::Owned(cleaned);
                }
                Some(Cow::Owned(palette.style(&style)))
            }
            (_, "bgcolor") => {
                palette.bgcolor(value);
                Some(Cow::Borrowed(value))
            }
            (_, "color") => {
                palette.color(value);
                Some(Cow::Borrowed(value))
            }
            _ => Some(Cow::Borrowed(value)),
        });
    builder.clean(html).to_string()
}

const PRINT_STYLE: &str = r#"
body { font: 13px/1.5 "Avenir", -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif; color: var(--emphasized-foreground); background: var(--background);
  margin: 24px; overflow-wrap: break-word; }
h1 { font-size: 18px; margin: 0 0 16px; }
.message { border-top: 1px solid var(--border); padding: 12px 0; }
.headers { color: var(--muted-foreground); margin-bottom: 12px; }
.headers b { color: var(--emphasized-foreground); }
.plain { white-space: pre-wrap; }
blockquote { margin: 0 0 0 2px; padding-left: 12px; border-left: 2px solid var(--border); color: var(--muted-foreground); }
details.quote > summary { display: none; }
img { max-width: 100%; height: auto; }
table { max-width: 100%; }
"#;

/// One message of a thread to print, its headers ready to show.
pub struct Printed<'a> {
    pub from: String,
    pub to: String,
    pub cc: String,
    pub date: String,
    pub body: Option<Body>,
    pub snippet: &'a str,
}

/// A whole thread as one page to print: every message open, with its headers, remote images left
/// out.
pub fn print(subject: &str, messages: &[Printed]) -> String {
    let subject = text::escape(subject);
    let palette = Arc::new(Palette::default());
    let mut html = format!("<h1>{subject}</h1>");
    for message in messages {
        let content = match &message.body {
            Some(Body { html: Some(body), .. }) => {
                sanitize(body, false, Arc::new(AtomicBool::new(false)), palette.clone())
            }
            Some(Body { text: Some(body), .. }) => {
                text::to_html(body).replace("<details class=\"quote\">", "<details class=\"quote\" open>")
            }
            _ => format!("<div class=\"plain\">{}</div>", text::escape(message.snippet)),
        };
        let mut headers = format!("<b>{}</b>", text::escape(&message.from));
        for (name, value) in [("To", &message.to), ("Cc", &message.cc), ("Date", &message.date)] {
            if !value.is_empty() {
                headers.push_str(&format!("<br>{name}: {}", text::escape(value)));
            }
        }
        html.push_str(&format!("<div class=\"message\"><div class=\"headers\">{headers}</div>{content}</div>"));
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{subject}</title>\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data: cid:; style-src 'unsafe-inline'\">\
         <style>{TOKENS}{PRINT_STYLE}{}</style></head><body class=\"light\">{html}</body></html>",
        palette.css()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(html: &str) -> Body {
        Body { html: Some(html.into()), text: None }
    }

    #[test]
    fn removes_scripts_and_handlers() {
        let page = page(
            &html("<p onclick=\"steal()\">Hi<script>alert(1)</script></p><a href=\"javascript:x()\">x</a>"),
            false,
            true,
        );
        assert!(!page.html.contains("steal"));
        assert!(!page.html.contains("alert"));
        assert!(!page.html.contains("javascript:"));
        assert!(page.html.contains("<p>Hi</p>"));
    }

    #[test]
    fn blocks_remote_images_until_asked() {
        let mail = html(
            "<img src=\"https://tracker.example/p.gif\"><div style=\"background:url(https://x.example/bg.png);color:red\">Hi</div><img src=\"data:image/png;base64,AA==\">",
        );
        let blocked = page(&mail, false, true);
        assert!(blocked.blocked_images);
        assert!(!blocked.html.contains("tracker.example"));
        assert!(!blocked.html.contains("x.example"));
        assert!(blocked.html.contains("color:var(--c0)") && blocked.html.contains("--c0:red;"));
        assert!(blocked.html.contains("data:image/png"));
        assert!(blocked.html.contains("img-src data: cid:;"));

        let shown = page(&mail, true, true);
        assert!(!shown.blocked_images);
        assert!(shown.html.contains("https://tracker.example/p.gif"));
    }

    #[test]
    fn designed_mail_is_drawn_on_paper() {
        assert!(
            page(&html("<table bgcolor=\"#eee\"><tr><td>Sale</td></tr></table>"), false, true)
                .html
                .contains("class=\"paper dark\"")
        );
        assert!(page(&html("<div>Hi Ann,<br>See you.</div>"), false, true).html.contains("class=\"themed dark\""));
        let plain = page(&Body { html: None, text: Some("Hi <b>".into()) }, false, true);
        assert!(plain.html.contains("class=\"themed dark\"") && plain.html.contains("Hi &lt;b&gt;"));
    }

    #[test]
    fn prints_every_message_open_with_its_headers() {
        let messages = [
            Printed {
                from: "Ann <ann@x.com>".into(),
                to: "me@example.com".into(),
                cc: String::new(),
                date: "Mar 3, 2026 at 9:41 AM".into(),
                body: Some(Body { html: None, text: Some("Lunch?\n\nOn Mar 2, Bob wrote:\n> Hungry".into()) }),
                snippet: "",
            },
            Printed {
                from: "Bob".into(),
                to: String::new(),
                cc: String::new(),
                date: String::new(),
                body: Some(html(
                    "<p style=\"color:#333\">Yes<img src=\"https://t.example/p.gif\"><script>x()</script></p>",
                )),
                snippet: "",
            },
            Printed {
                from: "Cy".into(),
                to: String::new(),
                cc: String::new(),
                date: String::new(),
                body: None,
                snippet: "On its way",
            },
        ];
        let page = print("Lunch <3", &messages);
        assert!(page.contains("<title>Lunch &lt;3</title>"));
        assert!(page.contains("<b>Ann &lt;ann@x.com&gt;</b><br>To: me@example.com<br>Date: Mar 3, 2026 at 9:41 AM"));
        assert!(page.contains("<details class=\"quote\" open>"));
        assert!(page.contains("<p style=\"color:var(--c0)\">Yes") && page.contains("--c0:#333;"));
        assert!(!page.contains("t.example") && !page.contains("x()"));
        assert!(page.contains("On its way"));
    }

    #[test]
    fn shapes_the_text_the_page_shows() {
        let texts = |lengths: &[u32]| lengths.iter().copied().map(Line::Text).collect::<Vec<_>>();
        let text = Body { html: None, text: Some("Sure.\n\nOn Mar 2, Bob wrote:\n> Lunch?\n> Friday\n\nSam".into()) };
        assert_eq!(shape(Some(&text), ""), texts(&[5, 0, 3, 3]));
        assert_eq!(
            shape(Some(&html("<head><title>Hi</title></head><p>Hi  Ann,</p><p>See you<br>soon</p>")), ""),
            texts(&[7, 0, 7, 4])
        );
        assert_eq!(shape(Some(&html("<div>Hi</div><div><br></div><div>Bye</div>")), ""), texts(&[2, 0, 3]));
        assert_eq!(shape(None, "See you at noon"), texts(&[15]));
        let cut = "word ".repeat(40);
        assert_eq!(shape(None, cut.trim()), texts(&[199, 0, 220, 160, 0, 90, 12]));
        assert_eq!(shape(None, "").len(), 7);
        let long = Body { html: None, text: Some("line\n".repeat(500)) };
        assert_eq!(shape(Some(&long), "").len(), SHAPE_LINES);
    }

    #[test]
    fn shapes_images_by_their_size() {
        let mail = html(
            "<img src=\"https://t.example/p.gif\" width=\"1\" height=\"1\"><table bgcolor=\"#fff\"><tr><td>Sale<img src=\"a.png\" WIDTH=600 height='300' style=\"max-height:9px\"></td></tr></table><img width=\"40\">",
        );
        let lines = shape(Some(&mail), "");
        assert_eq!(lines[..2], [Line::Text(4), Line::Image { width: 600, height: 300 }]);
        assert_eq!(lines.len(), DESIGNED_LINES, "designed mail is padded to its least height");
    }

    #[test]
    fn carries_the_theme() {
        assert!(page(&html("<p>x</p>"), false, true).html.contains("--background:"));
    }

    #[test]
    fn turns_the_mails_colours_for_the_dark_scheme() {
        let mail = html(
            "<table bgcolor=\"#ffffff\"><tr><td style=\"background:#f4f4f5;color:#222\"><font color=\"gray\">Sale</font></td></tr></table>",
        );
        let dark = page(&mail, false, true);
        assert!(dark.html.contains("class=\"paper dark\""));
        assert!(dark.html.contains("style=\"background:var(--c0);color:var(--c1)\""));
        assert!(dark.html.contains(":root{--c0:#f4f4f5;--c1:#222;}"));
        assert!(dark.html.contains("body.dark [bgcolor=\"#ffffff\" i]{background-color:#0f0f0f}"));
        assert!(dark.html.contains("body.dark [color=\"gray\" i]{color:"));
        assert!(dark.html.contains("bgcolor=\"#ffffff\""));

        let light = page(&mail, false, false);
        assert!(light.html.contains("class=\"paper light\""));
        assert!(!page(&html("<p>Hi</p>"), false, true).html.contains(":root{--c0"));
    }
}
