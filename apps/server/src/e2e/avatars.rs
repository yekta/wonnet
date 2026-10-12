use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{Html, IntoResponse};
use axum::routing::get;
use sqlx::PgPool;

use super::Server;
use crate::avatars::{public_client, signed_url};

fn png(side: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let icon = image::RgbaImage::from_pixel(side, side, image::Rgba([200, 30, 30, 255]));
    icon.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    bytes
}

/// The side of the square JPEG the server answered with.
fn side(bytes: &[u8]) -> u32 {
    let picture = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).unwrap();
    assert_eq!(picture.width(), picture.height());
    picture.width()
}

/// Gravatar knows ann@people.com. instagram.com's homepage names a large icon and small.com's a
/// small one; every site has an apple-touch-icon and a favicon too small to show. Notes every
/// request.
async fn fake_web(asked: Arc<Mutex<Vec<String>>>) -> String {
    let gravatar_asked = asked.clone();
    let gravatar = move |Path(hash): Path<String>| {
        gravatar_asked.lock().unwrap().push(format!("gravatar/{hash}"));
        let found = hash == crate::sha256_hex(b"ann@people.com");
        async move { found.then(|| ([(CONTENT_TYPE, "image/png")], png(256))).ok_or(StatusCode::NOT_FOUND) }
    };
    let site = move |Path(path): Path<String>| {
        asked.lock().unwrap().push(path.clone());
        async move {
            let page = r#"<link rel="icon" sizes="16x16" href="/instagram.com/16.png">
                <link rel="apple-touch-icon" sizes="180x180" href="/instagram.com/touch.png">"#;
            match path.as_str() {
                "instagram.com" => Ok(Html(page).into_response()),
                "small.com" => Ok(Html(r#"<link rel="icon" href="/small.com/64.png">"#).into_response()),
                "small.com/64.png" => Ok(png(64).into_response()),
                "instagram.com/touch.png" => Ok(png(180).into_response()),
                "apple-touch-icon.png" => Ok(png(120).into_response()),
                "favicon.ico" => Ok(png(16).into_response()),
                _ => Err(StatusCode::NOT_FOUND),
            }
        }
    };
    let app = Router::new().route("/gravatar/{hash}", get(gravatar)).route("/{*path}", get(site));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { axum::serve(listener, app.into_make_service()).await.unwrap() });
    format!("http://localhost:{port}")
}

async fn start(db: PgPool) -> (Server, Arc<Mutex<Vec<String>>>) {
    start_at(db, "localhost").await
}

async fn start_at(db: PgPool, host: &str) -> (Server, Arc<Mutex<Vec<String>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let base = fake_web(asked.clone()).await.replace("localhost", host);
    let server = Server::start_with(db, |config| {
        config.gravatar_url = format!("{base}/gravatar");
        config.site_url = format!("{base}/{{domain}}");
    })
    .await;
    (server, asked)
}

/// The side of `email`'s picture.
async fn picture(server: &Server, token: &str, email: &str) -> Result<u32, StatusCode> {
    let response = server.get(&format!("/api/avatar?email={email}"), Some(token)).await;
    if !response.status().is_success() {
        return Err(response.status());
    }
    Ok(side(&response.bytes().await.unwrap()))
}

#[sqlx::test]
async fn a_sender_gets_their_gravatar_or_else_their_companys_icon_asked_once(db: PgPool) {
    let (server, asked) = start(db).await;
    let (token, _, _) = server.gmail_user("me@gmail.com").await;

    assert_eq!(picture(&server, &token, "Ann@People.com").await, Ok(256));
    assert_eq!(picture(&server, &token, "no-reply@mail.instagram.com").await, Ok(180));
    assert_eq!(picture(&server, &token, "hello@plain.com").await, Ok(120));
    assert_eq!(picture(&server, &token, "hello@small.com").await, Ok(120));
    assert_eq!(picture(&server, &token, "bob@gmail.com").await, Err(StatusCode::NOT_FOUND));
    let first = asked.lock().unwrap().len();

    picture(&server, &token, "ann@people.com").await.unwrap();
    picture(&server, &token, "no-reply@mail.instagram.com").await.unwrap();
    assert_eq!(picture(&server, &token, "bob@gmail.com").await, Err(StatusCode::NOT_FOUND));
    let asked = asked.lock().unwrap();
    assert_eq!(asked.len(), first, "everything was kept: {asked:?}");
    assert!(!asked.iter().any(|path| path.starts_with("gmail.com")), "{asked:?}");
}

#[sqlx::test]
async fn a_picture_needs_a_session_or_the_signature_a_push_carries(db: PgPool) {
    let (server, _) = start(db).await;

    let unsigned = server.get("/api/avatar?email=ann@people.com", None).await;
    assert_eq!(unsigned.status(), StatusCode::UNAUTHORIZED);

    let signed = signed_url(&server.state, "Ann@people.com");
    let response = reqwest::get(&signed).await.unwrap();
    assert_eq!(side(&response.bytes().await.unwrap()), 256);

    let borrowed = signed.replace("ann%40people%2Ecom", "bob%40people%2Ecom");
    assert_ne!(borrowed, signed);
    assert_eq!(reqwest::get(&borrowed).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn pictures_are_never_fetched_from_our_own_network(db: PgPool) {
    let (server, asked) = start_at(db, "127.0.0.1").await;
    let (token, _, _) = server.gmail_user("me@gmail.com").await;
    assert_eq!(picture(&server, &token, "ann@people.com").await, Err(StatusCode::NOT_FOUND));
    assert_eq!(picture(&server, &token, "no-reply@instagram.com").await, Err(StatusCode::NOT_FOUND));
    assert!(asked.lock().unwrap().is_empty());

    let page = format!("{}/instagram.com", fake_web(Default::default()).await);
    assert!(reqwest::get(&page).await.unwrap().status().is_success());
    assert!(public_client().get(&page).send().await.is_err());
}

#[sqlx::test]
async fn senders_are_looked_up_as_their_mail_comes_and_once_when_asked_together(db: PgPool) {
    let (server, asked) = start(db).await;
    let (token, _, _) = server.gmail_user("me@gmail.com").await;

    crate::avatars::warm(&server.state, ["ann@people.com", "hello@plain.com"]);
    for _ in 0..100 {
        let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM avatars").fetch_one(&server.state.db).await.unwrap();
        if kept == 3 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let warmed = asked.lock().unwrap().len();
    assert_eq!(picture(&server, &token, "ann@people.com").await, Ok(256));
    assert_eq!(picture(&server, &token, "hello@plain.com").await, Ok(120));
    assert_eq!(asked.lock().unwrap().len(), warmed, "both were kept before they were asked for");

    let together = (0..5).map(|_| picture(&server, &token, "no-reply@instagram.com"));
    assert!(futures_util::future::join_all(together).await.iter().all(|side| *side == Ok(180)));
    let homepages = asked.lock().unwrap().iter().filter(|path| *path == "instagram.com").count();
    assert_eq!(homepages, 1);
}
