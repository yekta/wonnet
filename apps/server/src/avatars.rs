//! Senders' pictures: their Gravatar, or else their company's logo: the one its domain publishes
//! for mail (BIMI), or the icon its website gives itself. Each is fetched once for every user, as
//! soon as their mail comes, and kept, so neither Gravatar nor the company learns whose mail it is.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::io::Cursor;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::http::header::{ACCEPT, CACHE_CONTROL, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use hickory_resolver::TokioResolver;
use hickory_resolver::proto::rr::RData;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageReader, Limits, Rgba, RgbaImage};
use lol_html::{RewriteStrSettings, element, rewrite_str};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::redirect::Policy;
use reqwest::{StatusCode, Url};
use resvg::{tiny_skia, usvg};
use ring::hmac;
use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::error::{AppError, AppResult};
use crate::{AppState, api, sha256_hex};

pub const GRAVATAR_URL: &str = "https://gravatar.com/avatar";
pub const SITE_URL: &str = "https://{domain}";

/// Pixels on a side: the largest avatar at 3x.
const SIZE: u32 = 256;
/// The smallest icon worth showing, and one large enough to stop looking for a larger.
const SMALLEST: u32 = 32;
const LARGE_ENOUGH: u32 = 120;
const LARGEST_IMAGE: usize = 512 * 1024;
const LARGEST_PAGE: usize = 1024 * 1024;
/// Where an SVG icon, drawn for a browser's tab, ranks among sized ones: after an
/// apple-touch-icon, drawn as a tile.
const SVG_RANK: u32 = 150;
/// How many of the icons a homepage names are tried, largest first.
const MOST_NAMED: usize = 3;
/// How many senders are looked up at once in the background.
const WARMING: usize = 8;

/// The names of mail services whose addresses are people's own, not a company's.
const FREE_MAIL: &[&str] = &[
    "126",
    "163",
    "aol",
    "appleid",
    "att",
    "btinternet",
    "comcast",
    "duck",
    "fastmail",
    "free",
    "gmail",
    "gmx",
    "googlemail",
    "hey",
    "hotmail",
    "icloud",
    "inbox",
    "interia",
    "laposte",
    "libero",
    "live",
    "mac",
    "mail",
    "mailbox",
    "me",
    "msn",
    "naver",
    "o2",
    "orange",
    "outlook",
    "pm",
    "posteo",
    "proton",
    "protonmail",
    "qq",
    "rambler",
    "rediffmail",
    "rocketmail",
    "sbcglobal",
    "seznam",
    "t-online",
    "tuta",
    "tutanota",
    "verizon",
    "web",
    "wp",
    "yahoo",
    "yandex",
    "ymail",
    "zoho",
];

/// A picture as the apps get it, a square JPEG, and the side it was found at.
pub struct Image {
    bytes: Vec<u8>,
    side: u32,
}

/// What looking pictures up needs besides the table.
pub struct Avatars {
    /// Reaches only public addresses outside tests: a sender's website comes from mail anyone can
    /// send.
    http: reqwest::Client,
    /// Finds BIMI records; none in tests.
    dns: Option<TokioResolver>,
    /// One lookup of a key at a time: who comes second waits and finds it kept.
    looking: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// The senders looked up since the server started.
    warmed: Mutex<HashSet<String>>,
    warming: Arc<Semaphore>,
}

impl Avatars {
    pub fn new(http: reqwest::Client, dns: bool) -> Self {
        let dns = dns.then(|| TokioResolver::builder_tokio().map(|builder| builder.build())).and_then(Result::ok);
        Self {
            http,
            dns: dns.and_then(Result::ok),
            looking: Default::default(),
            warmed: Default::default(),
            warming: Arc::new(Semaphore::new(WARMING)),
        }
    }
}

#[derive(Deserialize)]
pub struct AvatarQuery {
    email: String,
    sig: Option<String>,
}

/// `GET /api/avatar?email=`: for a session, or with the signature a push carries.
pub async fn avatar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AvatarQuery>,
) -> AppResult<Response> {
    let email = query.email.trim().to_lowercase();
    if !query.sig.is_some_and(|sig| verify(&state, &email, &sig)) {
        api::user(&state, &headers).await?;
    }
    let Some(image) = find(&state, &email).await? else {
        return Err(AppError::not_found());
    };
    let headers = [(CONTENT_TYPE, "image/jpeg"), (CACHE_CONTROL, "private, max-age=604800")];
    Ok((headers, image).into_response())
}

/// The address of `email`'s picture for a device without a session: the notification extension.
pub fn signed_url(state: &AppState, email: &str) -> String {
    let email = email.trim().to_lowercase();
    let sig = hex::encode(hmac::sign(&key(state), email.as_bytes()));
    let email = utf8_percent_encode(&email, NON_ALPHANUMERIC);
    format!("{}/api/avatar?email={email}&sig={sig}", state.config.public_url)
}

fn verify(state: &AppState, email: &str, sig: &str) -> bool {
    hex::decode(sig).is_ok_and(|sig| hmac::verify(&key(state), email.as_bytes(), &sig).is_ok())
}

fn key(state: &AppState) -> hmac::Key {
    hmac::Key::new(hmac::HMAC_SHA256, format!("avatar:{}", state.config.secret_key).as_bytes())
}

/// Looks up the pictures of mail's senders in the background, so they are kept before anyone asks.
pub fn warm<'a>(state: &AppState, emails: impl IntoIterator<Item = &'a str>) {
    let new: Vec<String> = {
        let mut warmed = state.avatars.warmed.lock().unwrap();
        if warmed.len() > 100_000 {
            warmed.clear();
        }
        let emails = emails.into_iter().map(|email| email.trim().to_lowercase());
        emails.filter(|email| warmed.insert(email.clone())).collect()
    };
    for email in new {
        let state = state.clone();
        tokio::spawn(async move {
            let Ok(_turn) = state.avatars.warming.clone().acquire_owned().await else { return };
            if let Err(error) = find(&state, &email).await {
                tracing::warn!("couldn't look up a sender's picture: {error}");
            }
        });
    }
}

async fn find(state: &AppState, email: &str) -> sqlx::Result<Option<Vec<u8>>> {
    let avatars = &state.avatars;
    let hash = sha256_hex(email.as_bytes());
    let gravatar = format!("{}/{hash}?s={SIZE}&d=404", state.config.gravatar_url);
    let fetch = async { fetch_image(&avatars.http, Url::parse(&gravatar)?).await };
    let found = kept(state, &format!("gravatar:{hash}"), fetch).await?;
    if found.is_some() {
        return Ok(found);
    }
    let Some(domain) = company(email) else {
        return Ok(None);
    };
    let site = state.config.site_url.replace("{domain}", &domain);
    let fetch = async {
        if let Some(logo) = bimi(avatars, &domain).await {
            return Ok(Some(logo));
        }
        Ok(site_icon(&avatars.http, &site).await)
    };
    kept(state, &format!("company:{domain}"), fetch).await
}

/// What `key` was found to be, fetched again after a month, or a week when it was nothing. When
/// the fetch fails, what was found before.
async fn kept(
    state: &AppState,
    key: &str,
    fetch: impl Future<Output = anyhow::Result<Option<Image>>>,
) -> sqlx::Result<Option<Vec<u8>>> {
    let lock = state.avatars.looking.lock().unwrap().entry(key.to_string()).or_default().clone();
    let looking = lock.lock().await;
    let found = kept_or_fetched(state, key, fetch).await;
    drop(looking);
    state.avatars.looking.lock().unwrap().remove(key);
    found
}

async fn kept_or_fetched(
    state: &AppState,
    key: &str,
    fetch: impl Future<Output = anyhow::Result<Option<Image>>>,
) -> sqlx::Result<Option<Vec<u8>>> {
    let row: Option<(Option<Vec<u8>>, bool)> = sqlx::query_as(
        "SELECT image,
            fetched_at > now() - CASE WHEN image IS NULL THEN interval '7 days' ELSE interval '30 days' END
         FROM avatars WHERE key = $1",
    )
    .bind(key)
    .fetch_optional(&state.db)
    .await?;
    let fresh = row.as_ref().is_some_and(|(_, fresh)| *fresh);
    let before = row.and_then(|(image, _)| image);
    if fresh {
        return Ok(before);
    }
    let found = match fetch.await {
        Ok(found) => found.map(|image| image.bytes),
        Err(error) => {
            tracing::warn!("couldn't fetch the avatar {key}: {error:#}");
            return Ok(before);
        }
    };
    sqlx::query(
        "INSERT INTO avatars (key, image) VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET image = $2, fetched_at = now()",
    )
    .bind(key)
    .bind(&found)
    .execute(&state.db)
    .await?;
    Ok(found)
}

/// The logo `domain` publishes for mail in its BIMI record.
async fn bimi(avatars: &Avatars, domain: &str) -> Option<Image> {
    let found = avatars.dns.as_ref()?.txt_lookup(format!("default._bimi.{domain}.")).await.ok()?;
    let logo = found.answers().iter().find_map(|record| match &record.data {
        RData::TXT(text) => bimi_logo(&text.to_string()),
        _ => None,
    })?;
    fetch_image(&avatars.http, logo).await.ok().flatten()
}

/// The logo's address in a BIMI record: `v=BIMI1; l=https://…/logo.svg; a=…`.
fn bimi_logo(record: &str) -> Option<Url> {
    let mut tags =
        record.split(';').filter_map(|tag| tag.split_once('=')).map(|(name, value)| (name.trim(), value.trim()));
    tags.next().filter(|tag| *tag == ("v", "BIMI1"))?;
    let (_, logo) = tags.find(|(name, _)| *name == "l")?;
    Url::parse(logo).ok().filter(|url| url.scheme() == "https")
}

/// The largest of the icons `site`'s homepage names, its `/apple-touch-icon.png` and its
/// `/favicon.ico`, tried in that order until one is large enough. A site that can't be reached
/// has none.
async fn site_icon(http: &reqwest::Client, site: &str) -> Option<Image> {
    let mut root = Url::parse(site).ok()?;
    let mut tried = Vec::new();
    if let Ok((page, html)) = homepage(http, root.clone()).await {
        tried = named_icons(&page, &html).into_iter().take(MOST_NAMED).collect();
        root = page;
    }
    tried.extend(["/apple-touch-icon.png", "/favicon.ico"].iter().filter_map(|path| root.join(path).ok()));
    let mut best: Option<Image> = None;
    for url in tried {
        let Ok(Some(image)) = fetch_image(http, url).await else { continue };
        let side = image.side;
        if best.as_ref().is_none_or(|best| side > best.side) {
            best = Some(image);
        }
        if side >= LARGE_ENOUGH {
            break;
        }
    }
    best
}

async fn homepage(http: &reqwest::Client, url: Url) -> anyhow::Result<(Url, String)> {
    anyhow::ensure!(allowed(&url), "{url} is not a public address");
    let response = http.get(url).header(ACCEPT, "text/html").send().await?.error_for_status()?;
    let page = response.url().clone();
    let bytes = read(response, LARGEST_PAGE, true).await?;
    Ok((page, String::from_utf8_lossy(&bytes).into_owned()))
}

/// The icons a page names, largest first. An apple-touch-icon without sizes is 180 pixels.
fn named_icons(page: &Url, html: &str) -> Vec<Url> {
    let mut icons: Vec<(u32, Url)> = Vec::new();
    let settings = RewriteStrSettings::new().append_element_content_handler(element!("link[rel][href]", |link| {
        let rel = link.get_attribute("rel").unwrap_or_default().to_ascii_lowercase();
        let href = link.get_attribute("href").unwrap_or_default();
        let apple = rel.contains("apple-touch-icon");
        if !(apple || rel.split_whitespace().any(|word| word == "icon")) {
            return Ok(());
        }
        let svg = link.get_attribute("type").is_some_and(|kind| kind.contains("svg"))
            || href.split(['?', '#']).next().is_some_and(|path| path.ends_with(".svg"));
        let sizes = link.get_attribute("sizes").unwrap_or_default();
        let size = sizes.split(['x', 'X']).next().and_then(|side| side.trim().parse().ok());
        let size = match (svg, size) {
            (true, _) => SVG_RANK,
            (false, Some(size)) => size,
            (false, None) if apple => 180,
            (false, None) => 0,
        };
        if let Ok(url) = page.join(&href) {
            icons.push((size, url));
        }
        Ok(())
    }));
    let _ = rewrite_str(html, settings);
    icons.sort_by_key(|(size, _)| Reverse(*size));
    icons.into_iter().map(|(_, url)| url).collect()
}

/// The image at `url`, or `None` when there is none or it is too small to show.
async fn fetch_image(http: &reqwest::Client, url: Url) -> anyhow::Result<Option<Image>> {
    anyhow::ensure!(allowed(&url), "{url} is not a public address");
    let response = http.get(url).send().await?;
    if response.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    Ok(squared(&read(response.error_for_status()?, LARGEST_IMAGE, false).await?))
}

/// The picture as a square JPEG on white, at most `SIZE` on a side, or `None` when it isn't an
/// image or is too small to show.
fn squared(bytes: &[u8]) -> Option<Image> {
    let (mut picture, found) = match decoded(bytes) {
        Some(picture) => {
            let found = picture.width().min(picture.height());
            (picture, found)
        }
        None => (drawn(bytes)?, SIZE),
    };
    if found < SMALLEST {
        return None;
    }
    if picture.width().max(picture.height()) > SIZE {
        picture = picture.resize(SIZE, SIZE, FilterType::Lanczos3);
    }
    let picture = picture.to_rgba8();
    let side = picture.width().max(picture.height());
    let mut square = RgbaImage::from_pixel(side, side, Rgba([255, 255, 255, 255]));
    let (x, y) = ((side - picture.width()) / 2, (side - picture.height()) / 2);
    imageops::overlay(&mut square, &picture, x.into(), y.into());
    let mut jpeg = Vec::new();
    JpegEncoder::new_with_quality(&mut jpeg, 90).encode_image(&DynamicImage::ImageRgba8(square).to_rgb8()).ok()?;
    Some(Image { bytes: jpeg, side: found })
}

/// A PNG, JPEG, WebP, GIF or ICO. Decoding stops at a size no icon or photo has.
fn decoded(bytes: &[u8]) -> Option<DynamicImage> {
    let mut limits = Limits::default();
    (limits.max_image_width, limits.max_image_height, limits.max_alloc) = (Some(4096), Some(4096), Some(64 << 20));
    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    reader.limits(limits);
    reader.decode().ok()
}

/// An SVG drawn `SIZE` on its longer side. It may embed images as data, never read files.
fn drawn(bytes: &[u8]) -> Option<DynamicImage> {
    let resolver = usvg::ImageHrefResolver {
        resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
        resolve_string: Box::new(|_, _| None),
    };
    let options = usvg::Options { image_href_resolver: resolver, ..Default::default() };
    let tree = usvg::Tree::from_data(bytes, &options).ok()?;
    let scale = SIZE as f32 / tree.size().width().max(tree.size().height());
    let (width, height) = (tree.size().width() * scale, tree.size().height() * scale);
    let mut pixmap = tiny_skia::Pixmap::new(width.round().max(1.0) as u32, height.round().max(1.0) as u32)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    image::load_from_memory(&pixmap.encode_png().ok()?).ok()
}

/// The body, up to `largest` bytes: the start of a longer one when `truncate`, else an error.
async fn read(mut response: reqwest::Response, largest: usize, truncate: bool) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > largest {
            anyhow::ensure!(truncate, "{} is larger than {largest} bytes", response.url());
            bytes.truncate(largest);
            break;
        }
    }
    Ok(bytes)
}

/// The domain of the company an address belongs to: `mail.instagram.com` is `instagram.com`.
/// None for a free mail service's addresses.
fn company(email: &str) -> Option<String> {
    let host = email.rsplit_once('@')?.1;
    if !host.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'.' || byte == b'-') {
        return None;
    }
    let domain = psl::domain_str(host)?;
    let name = domain.split('.').next()?;
    (!FREE_MAIL.contains(&name)).then(|| domain.to_string())
}

/// The client that fetches pictures. A website's address comes from mail anyone can send, so it
/// connects only to public addresses: never to our own network.
pub fn public_client() -> reqwest::Client {
    let redirect = Policy::custom(|attempt| {
        if attempt.previous().len() >= 5 || !allowed(attempt.url()) {
            return attempt.stop();
        }
        attempt.follow()
    });
    reqwest::Client::builder()
        .dns_resolver(Arc::new(PublicOnly))
        .no_proxy()
        .redirect(redirect)
        .user_agent("Mozilla/5.0 (compatible; Wonnet; +https://wonnet.app)")
        .timeout(Duration::from_secs(10))
        .build()
        .expect("HTTP client")
}

/// An address the resolver doesn't see: http or https, to a name or a public IP.
fn allowed(url: &Url) -> bool {
    let host = match url.host() {
        Some(url::Host::Domain(_)) => true,
        Some(url::Host::Ipv4(ip)) => public(ip.into()),
        Some(url::Host::Ipv6(ip)) => public(ip.into()),
        None => false,
    };
    host && matches!(url.scheme(), "http" | "https")
}

struct PublicOnly;

impl Resolve for PublicOnly {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let found = tokio::net::lookup_host((name.as_str(), 0)).await?;
            let found: Vec<SocketAddr> = found.filter(|address| public(address.ip())).collect();
            if found.is_empty() {
                return Err(format!("{} has no public address", name.as_str()).into());
            }
            Ok(Box::new(found.into_iter()) as Addrs)
        })
    }
}

fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, ..] = ip.octets();
            let shared = a == 100 && (64..128).contains(&b);
            let benchmarking = a == 198 && (b == 18 || b == 19);
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || ip.is_unspecified()
                || ip.is_multicast()
                || a == 0
                || a >= 240
                || shared
                || benchmarking)
        }
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return public(ip.into());
            }
            let first = ip.segments()[0];
            let unique_local = first & 0xfe00 == 0xfc00;
            let link_local = first & 0xffc0 == 0xfe80;
            !(ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() || unique_local || link_local)
        }
    }
}

#[cfg(test)]
mod tests {
    use reqwest::Url;

    use super::{bimi_logo, company, named_icons, public, squared};

    #[test]
    fn a_company_is_the_domain_its_mail_comes_from() {
        assert_eq!(company("no-reply@mail.instagram.com").as_deref(), Some("instagram.com"));
        assert_eq!(company("news@bbc.co.uk").as_deref(), Some("bbc.co.uk"));
        assert_eq!(company("ann@gmail.com"), None);
        assert_eq!(company("ann@yahoo.co.uk"), None);
        assert_eq!(company("ann@example.com/../x"), None);
    }

    #[test]
    fn a_page_names_its_icons_largest_first() {
        let page = Url::parse("https://www.example.com/en/home").unwrap();
        let html = r#"<head>
            <link rel="shortcut icon" href="/favicon.ico">
            <link rel="icon" sizes="32x32" href="https://cdn.example.com/32.png">
            <link rel="icon" href="/favicon.svg">
            <link rel="apple-touch-icon" href="touch.png">
            <link rel="icon" sizes="512x512" href="/512.png?v=2">
            <link rel="stylesheet" href="/site.css">
        </head>"#;
        let icons: Vec<String> = named_icons(&page, html).iter().map(Url::to_string).collect();
        assert_eq!(
            icons,
            [
                "https://www.example.com/512.png?v=2",
                "https://www.example.com/en/touch.png",
                "https://www.example.com/favicon.svg",
                "https://cdn.example.com/32.png",
                "https://www.example.com/favicon.ico",
            ]
        );
    }

    #[test]
    fn a_bimi_record_gives_its_logo_over_https() {
        let record = "v=BIMI1; l=https://example.com/bimi/logo.svg; a=https://example.com/vmc.pem";
        assert_eq!(bimi_logo(record).unwrap().as_str(), "https://example.com/bimi/logo.svg");
        assert_eq!(bimi_logo("v=BIMI1; l=http://example.com/logo.svg"), None);
        assert_eq!(bimi_logo("v=spf1 l=https://example.com/logo.svg"), None);
    }

    #[test]
    fn an_svg_logo_is_drawn_square_on_white_without_reading_files() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10">
            <rect width="20" height="10" fill="#d00"/>
            <image href="/etc/hosts" width="20" height="10"/>
        </svg>"##;
        let image = squared(svg).unwrap();
        let drawn = image::load_from_memory(&image.bytes).unwrap().to_rgb8();
        assert_eq!(drawn.dimensions(), (256, 256));
        assert!(drawn.get_pixel(128, 0)[0] > 240 && drawn.get_pixel(128, 0)[1] > 240, "white above");
        assert!(drawn.get_pixel(128, 128)[0] > 180 && drawn.get_pixel(128, 128)[1] < 60, "red inside");
    }

    #[test]
    fn only_public_addresses_are_reached() {
        for ip in [
            "10.0.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "192.168.1.1",
            "100.64.0.1",
            "::1",
            "fd00::1",
            "::ffff:10.0.0.1",
        ] {
            assert!(!public(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["1.1.1.1", "142.250.74.14", "2606:4700::1111"] {
            assert!(public(ip.parse().unwrap()), "{ip}");
        }
    }
}
