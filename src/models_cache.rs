use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

const API_URL: &str = "https://models.dev/api.json";
const TTL_SECS: u64 = 300;

#[derive(Debug, Deserialize)]
struct ApiResponse(HashMap<String, ApiProvider>);

#[derive(Debug, Deserialize)]
struct ApiProvider {
    #[serde(default)]
    models: HashMap<String, ApiModel>,
}

#[derive(Debug, Deserialize)]
struct ApiModel {
    #[serde(default)]
    modalities: Option<ApiModalities>,
    #[serde(default)]
    limit: Option<ApiLimit>,
}

#[derive(Debug, Deserialize)]
struct ApiModalities {
    #[serde(default)]
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ApiLimit {
    #[serde(default)]
    context: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub input_modalities: Vec<String>,
    pub context_window: Option<u64>,
}

struct Cache {
    data: HashMap<String, HashMap<String, ModelInfo>>,
}

static CACHE: OnceLock<Mutex<Option<Cache>>> = OnceLock::new();

fn cache_lock() -> &'static Mutex<Option<Cache>> {
    CACHE.get_or_init(|| Mutex::new(None))
}

fn cache_file(paths: &crate::paths::MiyuPaths) -> PathBuf {
    paths.cache_dir.join("models_cache.json")
}

fn is_fresh(path: &PathBuf) -> bool {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let mtime = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };
    let elapsed = SystemTime::now()
        .duration_since(mtime)
        .unwrap_or(Duration::ZERO);
    elapsed.as_secs() < TTL_SECS
}

fn load_from_disk(path: &PathBuf) -> Result<HashMap<String, HashMap<String, ModelInfo>>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read models cache: {}", path.display()))?;
    let api: ApiResponse = serde_json::from_str(&text).context("failed to parse models cache")?;
    let mut result = HashMap::new();
    for (provider_id, provider) in api.0 {
        let mut models = HashMap::new();
        for (model_id, model) in provider.models {
            let input = model
                .modalities
                .map(|m| m.input)
                .unwrap_or_default();
            models.insert(model_id, ModelInfo {
                input_modalities: input,
                context_window: model.limit.and_then(|l| l.context),
            });
        }
        result.insert(provider_id, models);
    }
    Ok(result)
}

fn fetch_and_cache(path: &PathBuf) -> Result<HashMap<String, HashMap<String, ModelInfo>>> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()?;
    let text = client
        .get(API_URL)
        .header("User-Agent", "miyu")
        .send()?
        .text()?;
    if text.trim().is_empty() {
        anyhow::bail!("models.dev returned empty response");
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, &text)?;
    std::fs::rename(&temp, path)?;
    load_from_disk(path)
}

pub fn try_load(paths: &crate::paths::MiyuPaths) {
    let path = cache_file(paths);
    let data = if is_fresh(&path) {
        load_from_disk(&path).ok()
    } else {
        None
    };
    if let Some(data) = data {
        let mut lock = cache_lock().lock().unwrap();
        *lock = Some(Cache { data });
    }
}

pub fn spawn_background_refresh(paths: crate::paths::MiyuPaths) {
    let path = cache_file(&paths);
    std::thread::spawn(move || {
        let fetched = fetch_and_cache(&path).ok();
        if let Some(data) = fetched {
            let mut lock = cache_lock().lock().unwrap();
            *lock = Some(Cache { data });
        }
    });
}

pub fn supports_vision(provider_id: &str, model_id: &str) -> Option<bool> {
    let lock = cache_lock().lock().unwrap();
    let cache = lock.as_ref()?;
    let provider = cache.data.get(provider_id)?;
    let info = provider.get(model_id)?;
    Some(info.input_modalities.iter().any(|m| m == "image"))
}

pub fn context_window(model_id: &str) -> Option<u64> {
    let lock = cache_lock().lock().unwrap();
    let cache = lock.as_ref()?;
    for provider in cache.data.values() {
        if let Some(info) = provider.get(model_id) {
            return info.context_window;
        }
    }
    None
}

#[allow(dead_code)]
pub fn refresh_blocking(paths: &crate::paths::MiyuPaths) -> Result<()> {
    let path = cache_file(paths);
    let data = fetch_and_cache(&path)?;
    let mut lock = cache_lock().lock().unwrap();
    *lock = Some(Cache { data });
    Ok(())
}
