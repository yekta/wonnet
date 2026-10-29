//! What is sent: the text as written, the signature under it and the quote under that, as plain
//! text and as HTML.

use super::drafts::FORWARDED;
use super::text::linked;

fn present(part: Option<&str>) -> Option<&str> {
    part.map(str::trim_end).filter(|part| !part.trim().is_empty())
}

/// The text the message is sent with.
pub fn text(text: &str, signature: Option<&str>, quote: Option<&str>) -> String {
    let parts: Vec<&str> = [Some(text.trim_end()), present(signature), present(quote)].into_iter().flatten().collect();
    let mut joined = parts.join("\n\n");
    joined.push('\n');
    joined
}

/// The same as HTML: paragraphs with their links, the signature, and the quote, marked as Gmail
/// marks one so that mail apps fold it. `original` is the HTML of the message quoted: it is
/// quoted as its sender made it, under the quote's "wrote:" line, rather than as its text.
pub fn html(text: &str, signature: Option<&str>, quote: Option<&str>, original: Option<&str>) -> String {
    let mut html = format!("<div dir=\"auto\">{}</div>", blocks(text));
    if let Some(signature) = present(signature) {
        html.push_str(&format!("<div class=\"signature\" dir=\"auto\">{}</div>", blocks(signature)));
    }
    let Some(quote) = present(quote) else { return html };
    let Some(original) = original else {
        html.push_str(&format!("<div class=\"gmail_quote\" dir=\"auto\">{}</div>", blocks(quote)));
        return html;
    };
    let header: Vec<String> =
        quote.lines().take_while(|line| !line.trim().is_empty() && !is_quoted(line)).map(linked).collect();
    let header = format!("<div class=\"gmail_attr\" dir=\"auto\">{}<br></div>", header.join("<br>"));
    let original = match quote.starts_with(FORWARDED) {
        true => format!("<br>{original}"),
        false => {
            format!("<blockquote class=\"gmail_quote\" type=\"cite\" style=\"{QUOTE_STYLE}\">{original}</blockquote>")
        }
    };
    html.push_str(&format!("<div class=\"gmail_quote\">{header}{original}</div>"));
    html
}

const QUOTE_STYLE: &str = "margin:0 0 0 .8ex;border-left:1px solid #ccc;padding-left:1ex";

fn is_quoted(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

/// Paragraphs split by blank lines, single line breaks kept, quoted lines in a blockquote.
fn blocks(text: &str) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let mut out = String::new();
    let mut paragraph: Vec<String> = Vec::new();
    let flush = |paragraph: &mut Vec<String>, out: &mut String| {
        if !paragraph.is_empty() {
            out.push_str(&format!("<p style=\"margin:0 0 1em\">{}</p>", paragraph.join("<br>")));
            paragraph.clear();
        }
    };
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if is_quoted(line) {
            flush(&mut paragraph, &mut out);
            let start = index;
            while index < lines.len() && is_quoted(lines[index]) {
                index += 1;
            }
            let inner: Vec<&str> = lines[start..index]
                .iter()
                .map(|line| {
                    let line = line.trim_start().strip_prefix('>').unwrap_or(line);
                    line.strip_prefix(' ').unwrap_or(line)
                })
                .collect();
            out.push_str(&format!(
                "<blockquote type=\"cite\" style=\"{QUOTE_STYLE}\">{}</blockquote>",
                blocks(&inner.join("\n"))
            ));
            continue;
        }
        match line.trim().is_empty() {
            true => flush(&mut paragraph, &mut out),
            false => paragraph.push(linked(line)),
        }
        index += 1;
    }
    flush(&mut paragraph, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_puts_the_signature_then_the_quote_under_what_was_written() {
        assert_eq!(
            text("Sounds good.\n\n", Some("Sam\nAcme"), Some("On Mar 3, Ann wrote:\n> Lunch?")),
            "Sounds good.\n\nSam\nAcme\n\nOn Mar 3, Ann wrote:\n> Lunch?\n"
        );
        assert_eq!(text("Hi", Some("  "), None), "Hi\n");
    }

    #[test]
    fn html_escapes_links_and_keeps_paragraphs() {
        let html = html("Hi <Ann>,\nsee https://x.com/a?b=1&c=2\n\nBye", None, None, None);
        assert_eq!(
            html,
            "<div dir=\"auto\"><p style=\"margin:0 0 1em\">Hi &lt;Ann&gt;,<br>see <a href=\"https://x.com/a?b=1&amp;c=2\">https://x.com/a?b=1&amp;c=2</a></p>\
             <p style=\"margin:0 0 1em\">Bye</p></div>"
        );
    }

    #[test]
    fn html_quotes_in_a_blockquote_nested_as_written() {
        let html = html("Yes", Some("Sam"), Some("On Mar 3, Ann wrote:\n> Lunch?\n>> Earlier\n> Fri"), None);
        assert!(
            html.contains("<div class=\"signature\" dir=\"auto\"><p style=\"margin:0 0 1em\">Sam</p></div>"),
            "{html}"
        );
        assert!(html.contains("<p style=\"margin:0 0 1em\">On Mar 3, Ann wrote:</p><blockquote"), "{html}");
        assert!(html.contains("Lunch?</p><blockquote"), "{html}");
        assert!(html.contains("Earlier</p></blockquote><p style=\"margin:0 0 1em\">Fri</p></blockquote>"), "{html}");
    }

    #[test]
    fn html_quotes_the_original_as_its_sender_made_it() {
        let original = "<table><tr><td>Receipt</td><td>€10.00</td></tr></table>";
        let reply = html("Thanks", None, Some("On Mar 3, Stripe <a@b.com> wrote:\n> Receipt | €10.00"), Some(original));
        assert!(reply.ends_with(
            "<div class=\"gmail_quote\"><div class=\"gmail_attr\" dir=\"auto\">On Mar 3, Stripe &lt;<a href=\"mailto:a@b.com\">a@b.com</a>&gt; wrote:<br></div>\
             <blockquote class=\"gmail_quote\" type=\"cite\" style=\"margin:0 0 0 .8ex;border-left:1px solid #ccc;padding-left:1ex\"><table><tr><td>Receipt</td><td>€10.00</td></tr></table></blockquote></div>"
        ), "{reply}");
        let forward = html(
            "FYI",
            None,
            Some("---------- Forwarded message ----------\nFrom: Ann\n\nLunch?"),
            Some("<p>Lunch?</p>"),
        );
        assert!(
            forward.ends_with("Forwarded message ----------<br>From: Ann<br></div><br><p>Lunch?</p></div>"),
            "{forward}"
        );
    }
}
