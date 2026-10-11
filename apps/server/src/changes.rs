//! What changed for a user after a cursor, read in one snapshot. A batch holds at most
//! `BATCH_SIZE` messages; a full batch ends at its last message's rev, and the drafts it carries
//! end there too, so the next batch starts where this one stopped. Accounts, labels and
//! preferences are few and come whole with the first batch, so a new device lists every account
//! at once; `ahead` is the rev they were sent through, which later batches skip.

use mail_protocol::wire::ServerMessage;
use mail_protocol::{Account, BATCH_SIZE, Label};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::{ACCOUNT_COLUMNS, AccountRow, DraftRow, LabelRow, MESSAGE_COLUMNS, MessageRow, PreferenceRow};

pub async fn read(db: &PgPool, user_id: Uuid, cursor: i64, ahead: i64) -> sqlx::Result<ServerMessage> {
    let ahead = ahead.max(cursor);
    let mut tx = db.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY").execute(&mut *tx).await?;

    let rows: Vec<MessageRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {MESSAGE_COLUMNS} FROM messages WHERE user_id = $1 AND rev > $2 ORDER BY rev LIMIT $3"
    )))
    .bind(user_id)
    .bind(cursor)
    .bind(BATCH_SIZE as i64)
    .fetch_all(&mut *tx)
    .await?;
    let full = rows.len() == BATCH_SIZE;
    let upper = match (full, rows.last()) {
        (true, Some(last)) => last.rev,
        _ => i64::MAX,
    };

    let accounts: Vec<AccountRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {ACCOUNT_COLUMNS} FROM accounts WHERE user_id = $1 AND rev > $2 ORDER BY rev"
    )))
    .bind(user_id)
    .bind(ahead)
    .fetch_all(&mut *tx)
    .await?;
    let labels: Vec<LabelRow> = sqlx::query_as(
        "SELECT id, account_id, provider_id, name, rev, deleted
         FROM labels WHERE user_id = $1 AND rev > $2 ORDER BY rev",
    )
    .bind(user_id)
    .bind(ahead)
    .fetch_all(&mut *tx)
    .await?;
    let preferences: Vec<PreferenceRow> =
        sqlx::query_as("SELECT key, value, rev, deleted FROM preferences WHERE user_id = $1 AND rev > $2 ORDER BY rev")
            .bind(user_id)
            .bind(ahead)
            .fetch_all(&mut *tx)
            .await?;
    let drafts: Vec<DraftRow> = sqlx::query_as(
        "SELECT id, draft, updated, rev, deleted FROM drafts WHERE user_id = $1 AND rev > $2 AND rev <= $3 ORDER BY rev",
    )
    .bind(user_id)
    .bind(cursor)
    .bind(upper)
    .fetch_all(&mut *tx)
    .await?;
    let arriving: Vec<Uuid> = match full {
        true => {
            sqlx::query_scalar(
                "SELECT id FROM accounts WHERE user_id = $1 AND NOT deleted
                 AND EXISTS (SELECT 1 FROM messages WHERE account_id = accounts.id AND rev > $2)",
            )
            .bind(user_id)
            .bind(upper)
            .fetch_all(&mut *tx)
            .await?
        }
        false => Vec::new(),
    };
    tx.commit().await?;

    let highest = rows
        .iter()
        .map(|row| row.rev)
        .chain(accounts.iter().map(|row| row.rev))
        .chain(labels.iter().map(|row| row.rev))
        .chain(preferences.iter().map(|row| row.rev))
        .chain(drafts.iter().map(|row| row.rev))
        .max()
        .unwrap_or(cursor);
    let accounts: Vec<Account> = accounts.iter().map(AccountRow::wire).collect();
    let labels: Vec<Label> = labels.iter().map(LabelRow::wire).collect();
    Ok(ServerMessage::Changes {
        accounts,
        labels,
        messages: rows.into_iter().map(MessageRow::wire).collect(),
        preferences: preferences.into_iter().map(PreferenceRow::wire).collect(),
        drafts: drafts.into_iter().map(DraftRow::wire).collect(),
        arriving: arriving.iter().map(Uuid::to_string).collect(),
        cursor: if full { upper } else { highest.max(ahead) },
        more: full,
    })
}
