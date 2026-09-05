use std::sync::Mutex;
use std::time::{Duration, Instant};

const PROXMINT_URL: &str =
    "https://proxmint.com/api/free-proxies?protocol=https&format=txt";

const TEST_URL: &str =
    "https://character-tavern.com/api/homepage/cards?type=newest";

const PROXY_REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
const PROXY_TEST_TIMEOUT: Duration = Duration::from_secs(10);

struct ProxyCache {
    proxies: Vec<String>,
    refreshed_at: Option<Instant>,
}

static PROXY_CACHE: Mutex<ProxyCache> = Mutex::new(ProxyCache {
    proxies: Vec::new(),
    refreshed_at: None,
});

pub async fn get_working_client() -> Result<reqwest::Client, String> {
    let proxies = get_proxies().await?;

    for proxy in proxies {
        let proxy_url = format!("http://{}", proxy);

        let proxy_config = match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => proxy,
            Err(_) => continue,
        };

        let client = match reqwest::Client::builder()
            .proxy(proxy_config)
            .build()
        {
            Ok(client) => client,
            Err(_) => continue,
        };

        match client
            .get(TEST_URL)
            .timeout(PROXY_TEST_TIMEOUT)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                return Ok(client);
            }
            _ => continue,
        }
    }

    Err("No working proxy found".to_string())
}

async fn get_proxies() -> Result<Vec<String>, String> {
    {
        let cache = PROXY_CACHE
            .lock()
            .map_err(|_| "Proxy cache lock failed".to_string())?;

        if let Some(refreshed_at) = cache.refreshed_at {
            if refreshed_at.elapsed() < PROXY_REFRESH_INTERVAL
                && !cache.proxies.is_empty()
            {
                return Ok(cache.proxies.clone());
            }
        }
    }

    refresh_proxies().await
}

async fn refresh_proxies() -> Result<Vec<String>, String> {
    let response = reqwest::get(PROXMINT_URL)
        .await
        .map_err(|e| format!("Failed to fetch proxy list: {e}"))?;

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read proxy list: {e}"))?;

    let mut proxies = Vec::new();

    for line in text.lines() {
        let proxy = line.trim();

        if proxy.is_empty() {
            continue;
        }

        if !proxies.iter().any(|existing| existing == proxy) {
            proxies.push(proxy.to_string());
        }
    }

    if proxies.is_empty() {
        return Err("Proxy list was empty".to_string());
    }

    let mut cache = PROXY_CACHE
        .lock()
        .map_err(|_| "Proxy cache lock failed".to_string())?;

    cache.proxies = proxies.clone();
    cache.refreshed_at = Some(Instant::now());

    Ok(proxies)
}
