//! Gmail's IMAP, played over the fake Gmail's mailbox: All Mail numbers its messages oldest first,
//! and X-GM-MSGID is each one's API id read as hex.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::json;
use sqlx::PgPool;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use super::{FakeGoogle, RAW_MESSAGE, Server, matches, pass};
use crate::providers::Connection;

/// Asks `Server::start_with` for the fake IMAP.
pub const FAKE: &str = "fake";

pub async fn fake_imap(google: Arc<Mutex<FakeGoogle>>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("imap://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            tokio::spawn(serve(socket, google.clone()));
        }
    });
    url
}

async fn serve(socket: TcpStream, google: Arc<Mutex<FakeGoogle>>) {
    let (read, mut write) = socket.into_split();
    let mut lines = BufReader::new(read).lines();
    let _ = write.write_all(b"* OK Gimap ready\r\n").await;
    while let Ok(Some(line)) = lines.next_line().await {
        let (tag, command) = line.split_once(' ').unwrap_or((&line, ""));
        let reply = match command {
            "AUTHENTICATE XOAUTH2" => {
                let _ = write.write_all(b"+ \r\n").await;
                let token = lines.next_line().await.ok().flatten().unwrap_or_default();
                let token = String::from_utf8(STANDARD.decode(token).unwrap_or_default()).unwrap_or_default();
                if google.lock().unwrap().imap_refused || !token.contains("auth=Bearer access") {
                    let _ = write.write_all(b"+ eyJzdGF0dXMiOiI0MDAifQ==\r\n").await;
                    let _ = lines.next_line().await;
                    format!("{tag} NO [AUTHENTICATIONFAILED] Invalid credentials (Failure)\r\n")
                } else {
                    format!("{tag} OK ann@gmail.com authenticated (Success)\r\n")
                }
            }
            _ if command.starts_with("LIST") => format!(
                "* LIST (\\HasNoChildren) \"/\" \"INBOX\"\r\n\
                 * LIST (\\All \\HasNoChildren) \"/\" \"[Gmail]/All Mail\"\r\n{tag} OK Success\r\n"
            ),
            _ if command.starts_with("EXAMINE") => {
                let count = google.lock().unwrap().mailbox.len();
                format!(
                    "* FLAGS (\\Seen \\Flagged)\r\n* OK [UIDVALIDITY 7] UIDs valid.\r\n* {count} EXISTS\r\n\
                     * 0 RECENT\r\n* OK [UIDNEXT {}] Predicted next UID.\r\n\
                     {tag} OK [READ-ONLY] [Gmail]/All Mail selected. (Success)\r\n",
                    count + 1
                )
            }
            _ if command.starts_with("UID SEARCH ") => {
                let uids = search(&google.lock().unwrap(), &command["UID SEARCH ".len()..]);
                let uids: Vec<String> = uids.iter().map(u32::to_string).collect();
                format!("* SEARCH {}\r\n{tag} OK SEARCH completed (Success)\r\n", uids.join(" "))
            }
            _ if command.starts_with("UID FETCH ") && command.contains("BODY.PEEK[])") => {
                let reply = {
                    let mut google = google.lock().unwrap();
                    google.imap_stalled += usize::from(google.imap_stalls);
                    (!google.imap_stalls).then(|| fetch(&google, &command["UID FETCH ".len()..]))
                };
                let Some(mut reply) = reply else { continue };
                reply.extend(format!("{tag} OK Success\r\n").into_bytes());
                let _ = write.write_all(&reply).await;
                continue;
            }
            _ if command.starts_with("UID FETCH ") => {
                let mut reply = fetch(&google.lock().unwrap(), &command["UID FETCH ".len()..]);
                reply.extend(format!("{tag} OK Success\r\n").into_bytes());
                let _ = write.write_all(&reply).await;
                continue;
            }
            "LOGOUT" => format!("* BYE LOGOUT Requested\r\n{tag} OK 73 good day (Success)\r\n"),
            _ => format!("{tag} BAD Unknown command\r\n"),
        };
        let _ = write.write_all(reply.as_bytes()).await;
    }
}

/// The newest message has the highest UID.
fn uid(google: &FakeGoogle, index: usize) -> u32 {
    (google.mailbox.len() - index) as u32
}

fn search(google: &FakeGoogle, criteria: &str) -> Vec<u32> {
    if let Some(msg_id) = criteria.strip_prefix("X-GM-MSGID ") {
        let msg_id: u64 = msg_id.trim().parse().unwrap();
        let found = google.mailbox.iter().position(|(id, _)| u64::from_str_radix(id, 16).ok() == Some(msg_id));
        return found.map(|index| uid(google, index)).into_iter().collect();
    }
    let (within, query) = match criteria.strip_prefix("UID ") {
        Some(rest) => {
            let (set, query) = rest.split_once(' ').unwrap();
            let uids: Vec<u32> = match set.split_once(':') {
                Some((low, high)) => (low.parse().unwrap()..=high.parse().unwrap()).collect(),
                None => set.split(',').map(|uid| uid.parse().unwrap()).collect(),
            };
            (Some(uids), query)
        }
        None => (None, criteria),
    };
    let query = query.trim_start_matches("X-GM-RAW ").trim_matches('"');
    (0..google.mailbox.len())
        .filter(|index| matches(query, &google.mailbox[*index].1))
        .map(|index| uid(google, index))
        .filter(|uid| within.as_ref().is_none_or(|within| within.contains(uid)))
        .collect()
}

fn fetch(google: &FakeGoogle, arguments: &str) -> Vec<u8> {
    let (set, items) = arguments.split_once(' ').unwrap();
    let partial = items.split_once("BODY.PEEK[]<0.").map(|(_, rest)| rest.trim_end_matches(">)").parse().unwrap());
    let mut reply = Vec::new();
    for wanted in set.split(',').map(|uid| uid.parse::<u32>().unwrap()) {
        let Some(index) = (0..google.mailbox.len()).find(|index| uid(google, *index) == wanted) else { continue };
        let (id, labels) = &google.mailbox[index];
        let msg_id = u64::from_str_radix(id, 16).unwrap();
        let has = |label: &str| labels.iter().any(|other| other == label);
        let mut gmail_labels = Vec::new();
        for (label, imap) in [("INBOX", "\\Inbox"), ("SENT", "\\Sent"), ("STARRED", "\\Starred")] {
            if has(label) {
                gmail_labels.push(imap.to_string());
            }
        }
        let mut flags = Vec::new();
        if !has("UNREAD") {
            flags.push("\\Seen");
        }
        if has("STARRED") {
            flags.push("\\Flagged");
        }
        let date = chrono::DateTime::from_timestamp_millis(1_700_000_000_000 - index as i64 * 1000).unwrap();
        let raw = RAW_MESSAGE.as_bytes();
        let (section, data) = match partial {
            Some(bytes) => ("BODY[]<0>", &raw[..raw.len().min(bytes)]),
            None => ("BODY[]", raw),
        };
        reply.extend(
            format!(
                "* {wanted} FETCH (UID {wanted} X-GM-MSGID {msg_id} X-GM-THRID {msg_id} X-GM-LABELS ({}) FLAGS ({}) \
                 INTERNALDATE \"{}\" {section} {{{}}}\r\n",
                gmail_labels.join(" "),
                flags.join(" "),
                date.format("%d-%b-%Y %H:%M:%S +0000"),
                data.len()
            )
            .into_bytes(),
        );
        reply.extend(data);
        reply.extend(b")\r\n");
    }
    reply
}

/// Fifty messages, newest first: every other one in the inbox, every fourth unread, every tenth
/// a promotion and the newest starred.
fn mailbox() -> Vec<(String, Vec<String>)> {
    (0..50)
        .map(|index| {
            let mut labels = vec![if index % 2 == 0 { "INBOX" } else { "SENT" }.to_string()];
            if index % 4 == 0 {
                labels.push("UNREAD".into());
            }
            if index % 10 == 0 {
                labels.push("CATEGORY_PROMOTIONS".into());
            }
            if index == 0 {
                labels.push("STARRED".into());
            }
            (format!("{:x}", 0x18c0_0000 + 50 - index), labels)
        })
        .collect()
}

#[derive(sqlx::FromRow, Debug, PartialEq)]
struct Stored {
    provider_id: String,
    thread_id: String,
    labels: Vec<String>,
    unread: bool,
    starred: bool,
    bulk: bool,
    snippet: String,
    subject: String,
}

async fn stored(db: &PgPool) -> Vec<Stored> {
    sqlx::query_as(
        "SELECT provider_id, thread_id, labels, unread, starred, bulk, snippet, subject FROM messages ORDER BY date DESC",
    )
    .fetch_all(db)
    .await
    .unwrap()
}

#[sqlx::test]
async fn the_first_sync_comes_over_imap_and_spends_none_of_the_api_quota(db: PgPool) {
    let server = Server::start_with_imap(db.clone()).await;
    server.fill(mailbox());
    let (_, _, account) = server.gmail_user("ann@gmail.com").await;
    let connection = Connection::open(&server.state, &account).await.unwrap();
    let sync = async || pass(&server.state, &connection, account.id).await.unwrap();

    // The newest twenty of the inbox come first, as through the API.
    sync().await;
    let first = stored(&db).await;
    let inbox: Vec<String> = mailbox().into_iter().step_by(2).take(20).map(|(id, _)| id).collect();
    assert_eq!(first.iter().map(|message| message.provider_id.clone()).collect::<Vec<_>>(), inbox);

    for _ in 0..10 {
        let synced = sync().await;
        if !synced.syncing && !synced.backfilling {
            break;
        }
    }
    let all = stored(&db).await;
    assert_eq!(all.len(), 50);
    assert_eq!((server.fetched(), server.listed()), (vec![], vec![]), "nothing went through the API");
    let newest = &all[0];
    assert_eq!(newest.thread_id, newest.provider_id);
    assert_eq!(newest.labels, ["inbox"]);
    assert!(newest.unread && newest.starred && newest.bulk, "{newest:?}");
    assert_eq!((newest.subject.as_str(), newest.snippet.as_str()), ("Plan", "Here it is."));
    assert_eq!((all[1].labels.clone(), all[1].unread, all[1].bulk), (vec!["sent".to_string()], false, false));
    assert!(!all[2].unread && all[4].unread);

    // Bodies are fetched ahead over IMAP; one someone waits for comes through the API.
    let id = &newest.provider_id;
    assert_eq!(connection.background().raw(id).await.unwrap(), RAW_MESSAGE.as_bytes());
    assert!(server.fetched().is_empty());
    assert_eq!(connection.raw(id).await.unwrap(), RAW_MESSAGE.as_bytes());
    assert_eq!(server.fetched(), [format!("raw:{id}")]);
}

#[sqlx::test]
async fn the_api_syncs_when_imap_refuses(db: PgPool) {
    let server = Server::start_with_imap(db.clone()).await;
    server.fill(mailbox());
    server.refuse_imap();
    let (_, _, account) = server.gmail_user("ann@gmail.com").await;
    let connection = Connection::open(&server.state, &account).await.unwrap();
    for _ in 0..10 {
        let synced = pass(&server.state, &connection, account.id).await.unwrap();
        if !synced.syncing && !synced.backfilling {
            break;
        }
    }
    assert_eq!(stored(&db).await.len(), 50);
    assert_eq!(server.fetched().len(), 50, "every message came through the API");
}

#[sqlx::test]
async fn a_sync_asked_for_waits_on_new_mail_and_not_on_imap(db: PgPool) {
    let server = Server::start_with(db.clone(), |config| {
        config.gmail_imap_url = Some(FAKE.into());
        config.workers = true;
    })
    .await;
    server.fill(mailbox());
    server.stall_imap();
    let (_, user_id, account) = server.gmail_user("ann@gmail.com").await;
    server.state.workers.start(&server.state, account.id);
    for _ in 0..200 {
        if server.imap_stalled() > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(server.imap_stalled() > 0, "the worker fetches bodies ahead over IMAP");

    let new = format!("{:x}", 0x18c0_0000 + 51);
    let mut mailbox = mailbox();
    mailbox.insert(0, (new.clone(), vec!["INBOX".into()]));
    server.fill(mailbox);
    server.history(vec![json!({ "messagesAdded": [{ "message": { "id": new, "threadId": new } }] })]);

    let started = Instant::now();
    server.state.workers.sync_now(&server.state, user_id).await;
    assert!(started.elapsed() < Duration::from_secs(5), "took {:?}", started.elapsed());
    assert!(stored(&db).await.iter().any(|message| message.provider_id == new), "the new mail is in");
}
