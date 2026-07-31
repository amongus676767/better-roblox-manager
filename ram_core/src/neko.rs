//! Fetches a random SFW anime image for the corner overlay.
//!
//! Two sources are used at random:
//!
//! * **nekos.best** — actively maintained and returns artist attribution
//!   (`artist_name`, `artist_href`, `source_url`) alongside each image.
//! * **nekos.life** — older, no attribution metadata. Its maintainers removed
//!   the NSFW endpoints and images from the API, so only SFW content remains.
//!
//! Only the SFW `neko` endpoints are wired up here, deliberately and
//! exclusively. This ships inside an app aimed at Roblox players.
//!
//! Where attribution is available it is surfaced in the UI: these are working
//! artists' drawings, and an app that displays them should say whose they are.

use serde::Deserialize;

use crate::error::CoreError;

/// Refuse anything larger than this. A pathological GIF shouldn't be able to
/// stall the app or eat memory just because a stranger's API served it.
const MAX_IMAGE_BYTES: usize = 12 * 1024 * 1024;

const USER_AGENT: &str = concat!("BetterRobloxManager/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Clone)]
pub struct NekoImage {
    /// Raw encoded image bytes (PNG, JPEG or GIF).
    pub bytes: Vec<u8>,
    /// Artist name, when the source provides it.
    pub artist: Option<String>,
    /// Link to the original artwork, when the source provides it.
    pub source_url: Option<String>,
    /// Which API this came from, for display.
    pub source_site: &'static str,
}

#[derive(Deserialize)]
struct NekosLifeResponse {
    url: String,
}

#[derive(Deserialize)]
struct NekosBestResponse {
    results: Vec<NekosBestResult>,
}

#[derive(Deserialize)]
struct NekosBestResult {
    url: String,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    source_url: Option<String>,
}

/// Fetch one random SFW image. `pick` selects the source; pass anything, it is
/// reduced modulo the source count.
pub async fn fetch(pick: u32) -> Result<NekoImage, CoreError> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(20))
        .build()?;

    if pick % 2 == 0 {
        fetch_nekos_best(&client).await
    } else {
        fetch_nekos_life(&client).await
    }
}

async fn fetch_nekos_best(client: &reqwest::Client) -> Result<NekoImage, CoreError> {
    let meta: NekosBestResponse = client
        .get("https://nekos.best/api/v2/neko")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let first = meta
        .results
        .into_iter()
        .next()
        .ok_or_else(|| CoreError::AccountNotFound("nekos.best returned no results".into()))?;

    let bytes = download(client, &first.url).await?;
    Ok(NekoImage {
        bytes,
        artist: first.artist_name,
        source_url: first.source_url,
        source_site: "nekos.best",
    })
}

async fn fetch_nekos_life(client: &reqwest::Client) -> Result<NekoImage, CoreError> {
    let meta: NekosLifeResponse = client
        .get("https://nekos.life/api/v2/img/neko")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let bytes = download(client, &meta.url).await?;
    Ok(NekoImage {
        bytes,
        artist: None,
        source_url: None,
        source_site: "nekos.life",
    })
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, CoreError> {
    let resp = client.get(url).send().await?.error_for_status()?;

    // Reject on the advertised length first so an oversized file is refused
    // before we spend bandwidth on it.
    if let Some(len) = resp.content_length() {
        if len as usize > MAX_IMAGE_BYTES {
            return Err(CoreError::Crypto(format!(
                "image is {} MB, over the {} MB limit",
                len / (1024 * 1024),
                MAX_IMAGE_BYTES / (1024 * 1024)
            )));
        }
    }

    let bytes = resp.bytes().await?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(CoreError::Crypto("image exceeded the size limit".into()));
    }
    Ok(bytes.to_vec())
}
