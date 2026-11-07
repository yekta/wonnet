use serde_json::json;
use sqlx::PgPool;

use super::{Server, pass};
use crate::providers::Connection;

#[sqlx::test]
async fn label_changes_are_written_without_fetching_the_messages_again(db: PgPool) {
    let server = Server::start(db.clone()).await;
    let inbox = |id: &str, more: &[&str]| {
        let mut labels = vec!["INBOX".to_string()];
        labels.extend(more.iter().map(|label| label.to_string()));
        (id.to_string(), labels)
    };
    server.fill(vec![inbox("m0", &["UNREAD"]), inbox("m1", &[]), inbox("m2", &[]), inbox("m3", &[])]);
    let (_, _, account) = server.gmail_user("ann@gmail.com").await;
    let connection = Connection::open(&server.state, &account).await.unwrap();
    let sync = async || pass(&server.state, &connection, account.id).await.unwrap();
    while sync().await.syncing {}
    server.fetched();
    let state = async |id: &str| -> (Vec<String>, bool, bool, bool, i64) {
        sqlx::query_as("SELECT labels, unread, starred, bulk, rev FROM messages WHERE provider_id = $1")
            .bind(id)
            .fetch_one(&db)
            .await
            .unwrap()
    };
    let m3 = state("m3").await;

    server.fill(vec![
        inbox("new", &["UNREAD"]),
        inbox("m0", &[]),
        ("m1".into(), vec!["TRASH".into(), "STARRED".into()]),
        inbox("m2", &["CATEGORY_PROMOTIONS"]),
        inbox("m3", &[]),
    ]);
    let change = |id: &str, labels: &[&str]| json!([{ "message": { "id": id, "threadId": id }, "labelIds": labels }]);
    server.history(vec![
        json!({ "labelsRemoved": change("m0", &["UNREAD"]) }),
        json!({ "labelsAdded": change("m1", &["STARRED", "TRASH"]), "labelsRemoved": change("m1", &["INBOX"]) }),
        json!({ "messagesAdded": [{ "message": { "id": "new", "threadId": "new" } }] }),
        json!({ "labelsAdded": change("m2", &["CATEGORY_PROMOTIONS"]) }),
        json!({ "labelsAdded": change("m3", &["UNREAD"]) }),
        json!({ "labelsRemoved": change("m3", &["UNREAD"]) }),
    ]);
    sync().await;

    let mut fetched = server.fetched();
    fetched.sort();
    assert_eq!(fetched, ["m2", "new"], "only new mail, and mail moved between Gmail's tabs");
    assert!(!state("m0").await.1);
    let (labels, _, starred, ..) = state("m1").await;
    assert_eq!((labels, starred), (vec!["trash".to_string()], true));
    assert!(state("m2").await.3, "promotions are bulk");
    assert!(state("new").await.1);
    assert_eq!(state("m3").await, m3, "a message that ends up as it was keeps its rev");
}
