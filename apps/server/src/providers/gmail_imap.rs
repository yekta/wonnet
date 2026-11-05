//! Gmail over IMAP, for the work the API's per-user quota makes slow: listing and fetching a
//! mailbox a page at a time, and fetching bodies ahead. IMAP spends none of that quota. Gmail
//! gives each message its API ids (X-GM-MSGID and X-GM-THRID are them in decimal) and labels, so
//! what comes this way is the same as what comes through the API.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_imap::imap_proto::{AttributeValue, NameAttribute, Response, Status};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use futures_util::TryStreamExt;
use mail_protocol::{Attachment, role};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use uuid::Uuid;

use super::{one_line, unescape};
use crate::db::RemoteMessage;
use crate::mime;

pub const URL: &str = "imaps://imap.gmail.com:993";
/// How long any one step may take before the connection is given up on.
const TIMEOUT: Duration = Duration::from_secs(60);
/// How many UIDs a search looks through at once, so a large mailbox isn't listed whole each time.
const WINDOW: u32 = 20_000;
/// What Gmail files under its tabs for mail sent in bulk.
const BULK: &str = "category:promotions OR category:social OR category:updates OR category:forums";

trait Io: AsyncRead + AsyncWrite + Unpin + Send + std::fmt::Debug {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send + std::fmt::Debug> Io for T {}

type Session = async_imap::Session<Box<dyn Io>>;

pub struct Imap {
    session: Session,
    /// Changes when Gmail numbers the messages anew, which makes saved UIDs meaningless.
    pub uid_validity: u32,
    uid_next: u32,
}

/// A message as IMAP gives it: the start of its MIME, enough for its headers and snippet.
#[derive(Debug, Default)]
pub struct Fetched {
    pub uid: u32,
    pub msg_id: u64,
    pub thread_id: u64,
    pub labels: Vec<String>,
    pub flags: Vec<String>,
    pub internal_date: Option<i64>,
    pub raw: Vec<u8>,
}

struct XOAuth2(Option<String>);

impl async_imap::Authenticator for XOAuth2 {
    type Response = String;

    /// The token first; an empty answer to Gmail's error, after which it says NO.
    fn process(&mut self, _challenge: &[u8]) -> String {
        self.0.take().unwrap_or_default()
    }
}

async fn timed<T, E: Into<anyhow::Error>>(step: impl Future<Output = Result<T, E>>) -> anyhow::Result<T> {
    match tokio::time::timeout(TIMEOUT, step).await {
        Ok(result) => result.map_err(Into::into),
        Err(_) => anyhow::bail!("Gmail's IMAP took too long"),
    }
}

impl Imap {
    /// Signs in with the API's access token and opens All Mail to read.
    pub async fn connect(url: &str, address: &str, access_token: &str) -> anyhow::Result<Self> {
        let (secure, host_port) = match url.split_once("://") {
            Some(("imaps", rest)) => (true, rest),
            Some(("imap", rest)) => (false, rest),
            _ => anyhow::bail!("{url} isn't an IMAP URL"),
        };
        let host = host_port.rsplit_once(':').map_or(host_port, |(host, _)| host).to_string();
        let tcp = timed(TcpStream::connect(host_port)).await?;
        let stream: Box<dyn Io> = match secure {
            true => {
                let roots = tokio_rustls::rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
                let config =
                    tokio_rustls::rustls::ClientConfig::builder().with_root_certificates(roots).with_no_client_auth();
                let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
                Box::new(timed(connector.connect(host.try_into()?, tcp)).await?)
            }
            false => Box::new(tcp),
        };
        let mut client = async_imap::Client::new(stream);
        timed(client.read_response()).await?;
        let token = format!("user={address}\x01auth=Bearer {access_token}\x01\x01");
        let login = client.authenticate("XOAUTH2", XOAuth2(Some(token)));
        let mut session = match tokio::time::timeout(TIMEOUT, login).await {
            Ok(Ok(session)) => session,
            Ok(Err((error, _))) => anyhow::bail!("Gmail's IMAP refused the sign-in: {error}"),
            Err(_) => anyhow::bail!("Gmail's IMAP took too long to sign in"),
        };
        let names: Vec<_> = timed(async { session.list(Some(""), Some("*")).await?.try_collect().await }).await?;
        let Some(all) = names.iter().find(|name| name.attributes().contains(&NameAttribute::All)) else {
            anyhow::bail!("Gmail's IMAP shows no All Mail");
        };
        let all = all.name().to_string();
        let mailbox = timed(session.examine(&all)).await?;
        let Some(uid_next) = mailbox.uid_next else { anyhow::bail!("Gmail's IMAP gave no UIDNEXT") };
        Ok(Self { session, uid_validity: mailbox.uid_validity.unwrap_or_default(), uid_next })
    }

    /// The UIDs of the newest `count` messages below `below` a Gmail search finds, newest first,
    /// and whether it finds more.
    pub async fn search(&mut self, query: &str, below: Option<u32>, count: usize) -> anyhow::Result<(Vec<u32>, bool)> {
        let mut uids = Vec::new();
        let mut high = below.unwrap_or(self.uid_next).saturating_sub(1);
        while high > 0 && uids.len() <= count {
            let low = high.saturating_sub(WINDOW - 1).max(1);
            let command = format!("UID {low}:{high} X-GM-RAW {}", quote(query));
            let mut found: Vec<u32> = timed(self.session.uid_search(command)).await?.into_iter().collect();
            found.sort_unstable_by(|a, b| b.cmp(a));
            uids.extend(found);
            high = low - 1;
        }
        let more = uids.len() > count;
        uids.truncate(count);
        Ok((uids, more))
    }

    /// Which of `uids` Gmail files under its tabs for bulk mail.
    pub async fn bulk(&mut self, uids: &[u32]) -> anyhow::Result<HashSet<u32>> {
        let command = format!("UID {} X-GM-RAW {}", uid_set(uids), quote(BULK));
        Ok(timed(self.session.uid_search(command)).await?.into_iter().collect())
    }

    /// The messages with the first `bytes` of each, or all of it.
    pub async fn fetch(&mut self, uids: &[u32], bytes: Option<u32>) -> anyhow::Result<Vec<Fetched>> {
        if uids.is_empty() {
            return Ok(Vec::new());
        }
        let body = bytes.map_or("BODY.PEEK[]".to_string(), |bytes| format!("BODY.PEEK[]<0.{bytes}>"));
        let command =
            format!("UID FETCH {} (UID X-GM-MSGID X-GM-THRID X-GM-LABELS FLAGS INTERNALDATE {body})", uid_set(uids));
        let wanted: HashSet<u32> = uids.iter().copied().collect();
        let tag = timed(self.session.run_command(&command)).await?;
        let mut fetched = Vec::new();
        loop {
            let Some(response) = timed(self.session.read_response()).await? else {
                anyhow::bail!("Gmail's IMAP closed the connection");
            };
            match response.parsed() {
                Response::Fetch(_, attributes) => {
                    let message = fetched_of(attributes);
                    if wanted.contains(&message.uid) && message.msg_id != 0 {
                        fetched.push(message);
                    }
                }
                Response::Done { tag: done, status, outcome } if *done == tag => {
                    if *status != Status::Ok {
                        anyhow::bail!("Gmail's IMAP refused a fetch: {}", outcome.information.as_deref().unwrap_or(""));
                    }
                    return Ok(fetched);
                }
                _ => {}
            }
        }
    }

    /// A message's whole MIME by its API id, if it is in All Mail.
    pub async fn raw(&mut self, provider_id: &str) -> anyhow::Result<Option<Vec<u8>>> {
        let msg_id = u64::from_str_radix(provider_id, 16)?;
        let found = timed(self.session.uid_search(format!("X-GM-MSGID {msg_id}"))).await?;
        let Some(uid) = found.into_iter().next() else { return Ok(None) };
        let message = self.fetch(&[uid], None).await?.into_iter().find(|message| message.msg_id == msg_id);
        Ok(message.map(|message| message.raw))
    }

    pub async fn logout(mut self) {
        let _ = tokio::time::timeout(Duration::from_secs(5), self.session.logout()).await;
    }
}

fn fetched_of(attributes: &[AttributeValue<'_>]) -> Fetched {
    let mut message = Fetched::default();
    for attribute in attributes {
        match attribute {
            AttributeValue::Uid(uid) => message.uid = *uid,
            AttributeValue::GmailMsgId(id) => message.msg_id = *id,
            AttributeValue::GmailThrId(id) => message.thread_id = *id,
            AttributeValue::GmailLabels(labels) => message.labels = labels.iter().map(|label| decode(label)).collect(),
            AttributeValue::Flags(flags) => message.flags = flags.iter().map(|flag| flag.to_string()).collect(),
            AttributeValue::InternalDate(date) => message.internal_date = internal_date(date),
            AttributeValue::BodySection { section: None, data: Some(data), .. } => message.raw = data.to_vec(),
            _ => {}
        }
    }
    message
}

/// The message as the server keeps it. `labels` are the custom labels by name.
pub fn remote(message: Fetched, labels: &HashMap<String, Uuid>, bulk: bool) -> RemoteMessage {
    let headers = mime::headers(&header_lines(&message.raw));
    let snippet = mime::parse(&message.raw).map(|parsed| parsed.snippet).unwrap_or_default();
    let flag = |wanted: &str| message.flags.iter().any(|flag| flag.eq_ignore_ascii_case(wanted));
    let mut starred = flag("\\Flagged");
    let mut normalized = Vec::new();
    for label in &message.labels {
        match label.to_ascii_lowercase().as_str() {
            "\\inbox" => normalized.push(role::INBOX.to_string()),
            "\\sent" => normalized.push(role::SENT.to_string()),
            "\\draft" => normalized.push(role::DRAFTS.to_string()),
            "\\trash" => normalized.push(role::TRASH.to_string()),
            "\\spam" => normalized.push(role::SPAM.to_string()),
            "\\starred" => starred = true,
            _ => normalized.extend(labels.get(label).map(Uuid::to_string)),
        }
    }
    let attachments = match headers.multipart_mixed {
        true => vec![Attachment { name: String::new(), mime: String::new(), size: 0 }],
        false => Vec::new(),
    };
    RemoteMessage {
        provider_id: format!("{:x}", message.msg_id),
        thread_id: format!("{:x}", message.thread_id),
        from: headers.from,
        recipients: headers.recipients,
        subject: headers.subject,
        snippet: one_line(&unescape(&snippet)),
        date: message.internal_date.or(headers.date).unwrap_or(0),
        unread: !flag("\\Seen"),
        starred,
        labels: normalized,
        attachments,
        message_id: headers.message_id,
        in_reply_to: headers.in_reply_to,
        references: headers.references,
        bulk: bulk || headers.bulk,
        unsubscribe: headers.unsubscribe,
    }
}

/// The header lines at the start of a message, each unfolded.
fn header_lines(raw: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(raw);
    let head = text.split("\r\n\r\n").next().unwrap_or_default();
    let mut lines: Vec<(String, String)> = Vec::new();
    for line in head.split("\r\n") {
        if line.starts_with([' ', '\t']) {
            if let Some((_, value)) = lines.last_mut() {
                value.push(' ');
                value.push_str(line.trim());
            }
            continue;
        }
        let Some((name, value)) = line.split_once(':') else { continue };
        lines.push((name.trim().to_string(), value.trim().to_string()));
    }
    lines
}

/// IMAP's date, as in `05-Nov-2026 10:00:00 +0000`, in milliseconds.
fn internal_date(date: &str) -> Option<i64> {
    let date = chrono::DateTime::parse_from_str(date.trim(), "%d-%b-%Y %H:%M:%S %z").ok()?;
    Some(date.timestamp_millis())
}

fn quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn uid_set(uids: &[u32]) -> String {
    uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

/// A label name as Gmail sends it: quoted with its escapes kept, in IMAP's modified UTF-7, where
/// `&` opens UTF-16 in base64 with `,` for `/`, `-` closes it, and `&-` is `&`.
fn decode(name: &str) -> String {
    let name = name.replace("\\\\", "\\").replace("\\\"", "\"");
    let name = name.as_str();
    let mut decoded = String::new();
    let mut rest = name;
    while let Some(start) = rest.find('&') {
        decoded.push_str(&rest[..start]);
        let Some(length) = rest[start + 1..].find('-') else { return name.to_string() };
        let encoded = &rest[start + 1..start + 1 + length];
        rest = &rest[start + 2 + length..];
        if encoded.is_empty() {
            decoded.push('&');
            continue;
        }
        let mut padded = encoded.replace(',', "/");
        while !padded.len().is_multiple_of(4) {
            padded.push('=');
        }
        let Ok(bytes) = STANDARD.decode(padded) else { return name.to_string() };
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
        decoded.push_str(&String::from_utf16_lossy(&units));
    }
    decoded.push_str(rest);
    decoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_label_names() {
        assert_eq!(decode("Work/Plans"), "Work/Plans");
        assert_eq!(decode("Tom &- Jerry"), "Tom & Jerry");
        assert_eq!(decode("&AOk-t&AOk-"), "été");
        assert_eq!(decode("&ZeVnLIqe-"), "日本語");
        assert_eq!(decode(r#"Say \"hi\""#), r#"Say "hi""#);
    }

    #[test]
    fn a_fetched_message_is_what_the_api_would_give() {
        let custom = Uuid::new_v4();
        let labels = HashMap::from([("Work/Plans".to_string(), custom)]);
        let raw = "From: Ann <ann@example.com>\r\nTo: bo@example.com\r\nSubject: Long\r\n subject\r\n\
            Content-Type: text/plain\r\n\r\nHello there."
            .as_bytes()
            .to_vec();
        let message = Fetched {
            uid: 9,
            msg_id: 0x18c2f,
            thread_id: 0x18c00,
            labels: vec!["\\Inbox".into(), "\\Important".into(), "Work/Plans".into()],
            flags: vec!["\\Flagged".into()],
            internal_date: internal_date(" 5-Nov-2026 10:00:00 +0000"),
            raw,
        };
        let remote = remote(message, &labels, true);
        assert_eq!((remote.provider_id.as_str(), remote.thread_id.as_str()), ("18c2f", "18c00"));
        assert_eq!(remote.labels, ["inbox".to_string(), custom.to_string()]);
        assert!(remote.unread && remote.starred && remote.bulk);
        assert_eq!(remote.subject, "Long subject");
        assert_eq!(remote.from.email, "ann@example.com");
        assert_eq!(remote.snippet, "Hello there.");
        assert_eq!(remote.date, 1_793_872_800_000);
    }
}
