use sqlx::PgPool;

use super::Server;
use crate::db;
use crate::providers::Connection;

/// Fifty messages at Gmail, newest first; every other one is in the inbox.
fn mailbox() -> Vec<(String, Vec<String>)> {
    (0..50)
        .map(|index| {
            let labels = match index % 2 {
                0 => vec!["INBOX".to_string()],
                _ => vec!["SENT".to_string()],
            };
            (format!("m{index}"), labels)
        })
        .collect()
}

#[sqlx::test]
async fn the_inbox_comes_first_a_page_at_a_time_and_a_failed_pass_carries_on(db: PgPool) {
    let server = Server::start(db.clone()).await;
    server.fill(mailbox());
    let (_, _, account) = server.gmail_user("ann@gmail.com").await;
    let connection = Connection::open(&server.state, &account).await.unwrap();
    let sync = async || {
        let account = db::account(&db, account.id).await.unwrap().unwrap();
        connection.background().sync(&server.state, &account).await
    };
    // What is stored, in the order it came, as the pages it came in.
    let stored = async |pages: &[usize]| -> Vec<Vec<String>> {
        let all: Vec<String> =
            sqlx::query_scalar("SELECT provider_id FROM messages WHERE account_id = $1 ORDER BY rev")
                .bind(account.id)
                .fetch_all(&db)
                .await
                .unwrap();
        let mut rest = all.as_slice();
        let mut split = Vec::new();
        for page in pages {
            let (page, after) = rest.split_at(*page);
            let mut page = page.to_vec();
            page.sort();
            split.push(page);
            rest = after;
        }
        assert!(rest.is_empty(), "more than expected: {rest:?}");
        split
    };
    let ids = |range: std::ops::Range<usize>, step: usize| -> Vec<String> {
        let mut ids: Vec<String> = range.step_by(step).map(|index| format!("m{index}")).collect();
        ids.sort();
        ids
    };

    // The newest twenty of the inbox land first, then the rest of the inbox.
    assert!(sync().await.unwrap().syncing);
    assert_eq!(stored(&[20]).await, [ids(0..40, 2)]);
    assert!(sync().await.unwrap().syncing);
    assert_eq!(stored(&[20, 5]).await, [ids(0..40, 2), ids(40..50, 2)]);

    // A pass that fails leaves what it had and the next picks up where it was.
    server.refuse_fetches(1);
    assert!(sync().await.is_err());
    assert_eq!(stored(&[20, 5]).await, [ids(0..40, 2), ids(40..50, 2)]);
    assert!(!sync().await.unwrap().syncing);
    assert_eq!(stored(&[20, 5, 25]).await, [ids(0..40, 2), ids(40..50, 2), ids(1..50, 2)]);

    let recent = "newer_than:90d";
    assert_eq!(
        server.listed(),
        [
            (format!("{recent} in:inbox"), None),
            (format!("{recent} in:inbox"), Some("20".to_string())),
            (format!("{recent} -in:inbox"), None),
            (format!("{recent} -in:inbox"), None),
        ]
    );

    // Older mail follows once the recent is listed, and the account is ready meanwhile.
    let synced = sync().await.unwrap();
    assert!(!synced.backfilling && !synced.syncing);
    assert_eq!(server.listed().last().unwrap().0, "older_than:90d");
}
