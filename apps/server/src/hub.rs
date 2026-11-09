//! Wakes a user's sockets when their data changed. Writers call `pg_notify('changes', user)`;
//! one LISTEN connection per process passes it on, so it works across replicas. The same
//! connection wakes account workers on `ops`, tells the pusher of changes, and tells the sockets
//! when an application got or lost access to the user's mail, on `apps`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::postgres::PgListener;
use tokio::sync::watch;
use uuid::Uuid;

use crate::AppState;

/// How many times a user's data and their connected apps changed; a socket compares them with
/// the counts it saw last.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Pokes {
    pub changes: u64,
    pub apps: u64,
}

#[derive(Default)]
pub struct Hub {
    users: Mutex<HashMap<Uuid, watch::Sender<Pokes>>>,
}

impl Hub {
    pub fn subscribe(&self, user_id: Uuid) -> watch::Receiver<Pokes> {
        let mut users = self.users.lock().unwrap();
        users.retain(|_, sender| sender.receiver_count() > 0);
        users.entry(user_id).or_insert_with(|| watch::channel(Pokes::default()).0).subscribe()
    }

    pub fn poke(&self, user_id: Uuid) {
        self.modify(user_id, |pokes| pokes.changes += 1);
    }

    fn poke_apps(&self, user_id: Uuid) {
        self.modify(user_id, |pokes| pokes.apps += 1);
    }

    fn modify(&self, user_id: Uuid, change: impl FnOnce(&mut Pokes)) {
        let users = self.users.lock().unwrap();
        let Some(sender) = users.get(&user_id) else { return };
        sender.send_modify(change);
    }

    /// After the connection was down: anything could have changed.
    fn poke_all(&self) {
        for sender in self.users.lock().unwrap().values() {
            sender.send_modify(|pokes| {
                pokes.changes += 1;
                pokes.apps += 1;
            });
        }
    }
}

pub async fn notify_ops(db: &PgPool, account_id: Uuid) {
    let _ = sqlx::query("SELECT pg_notify('ops', $1)").bind(account_id.to_string()).execute(db).await;
}

/// Tells the user's sockets their connected apps changed, once `tx` commits.
pub async fn notify_apps(tx: &mut sqlx::PgConnection, user_id: Uuid) -> sqlx::Result<()> {
    sqlx::query("SELECT pg_notify('apps', $1)").bind(user_id.to_string()).execute(tx).await?;
    Ok(())
}

/// Listens on a connection of its own, outside the pool the requests use.
pub fn listen(state: AppState) {
    tokio::spawn(async move {
        let pool = state.background.clone();
        let mut stopping = state.stopping.subscribe();
        loop {
            tokio::select! {
                result = listen_once(&state, &pool) => {
                    if let Err(error) = result {
                        tracing::warn!("LISTEN failed: {error}");
                    }
                }
                _ = async { let _ = stopping.wait_for(|stopping| *stopping).await; } => {
                    pool.close().await;
                    return;
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn listen_once(state: &AppState, pool: &PgPool) -> sqlx::Result<()> {
    let mut listener = PgListener::connect_with(pool).await?;
    listener.listen_all(["changes", "ops", "apps"]).await?;
    // Anything missed while the connection was down.
    state.hub.poke_all();
    loop {
        let notification = listener.recv().await?;
        let Ok(id) = notification.payload().parse::<Uuid>() else { continue };
        match notification.channel() {
            "changes" => {
                state.hub.poke(id);
                state.pusher.changed(id);
            }
            "apps" => state.hub.poke_apps(id),
            _ => state.workers.wake(id),
        }
    }
}
