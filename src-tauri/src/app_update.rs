//! 应用自身更新。
//!
//! GitHub 始终是正式发布源；可在正式构建时通过环境变量加入 Gitee 国内镜像。
//! 两个来源分别检查、分别下载，下载内容最终都由 Tauri 的内置签名验证把关。

use crate::{config, i18n};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::PublicKey;
use semver::Version;
use serde::Serialize;
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

const GITHUB_UPDATE_ENDPOINT: &str =
    "https://github.com/lh0227-tech/TempleFix/releases/latest/download/latest.json";
const UPDATER_PUBLIC_KEY: &str = include_str!("../updater-public.key");
const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;
const AUTO_CHECK_DELAY_SECS: u64 = 15;
const SOURCE_TIMEOUT_SECS: u64 = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
struct UpdateSource {
    name: &'static str,
    endpoint: String,
}

#[derive(Clone)]
struct PendingUpdate {
    update: Update,
    source: UpdateSource,
    rank: usize,
}

pub struct AppUpdateState {
    checking: AtomicBool,
    pending: Mutex<Vec<PendingUpdate>>,
    last_status: Mutex<Option<AppUpdateStatus>>,
}

impl Default for AppUpdateState {
    fn default() -> Self {
        Self {
            checking: AtomicBool::new(false),
            pending: Mutex::new(Vec::new()),
            last_status: Mutex::new(None),
        }
    }
}

impl AppUpdateState {
    pub fn available_version(&self) -> Option<String> {
        self.pending
            .lock()
            .ok()
            .and_then(|pending| pending.first().map(|item| item.update.version.clone()))
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateStatus {
    pub configured: bool,
    pub gitee_configured: bool,
    pub current_version: String,
    pub checking: bool,
    pub available: bool,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub source: Option<String>,
    pub last_checked_at: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    stage: &'static str,
    source: String,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    percent: u8,
}

struct CheckingGuard<'a>(&'a AtomicBool);

impl Drop for CheckingGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn is_auto_check_due(last_checked_at: u64, now: u64) -> bool {
    last_checked_at == 0 || now.saturating_sub(last_checked_at) >= CHECK_INTERVAL_SECS
}

fn release_pubkey() -> Option<&'static str> {
    let value = UPDATER_PUBLIC_KEY.trim();
    (!value.contains("PLACEHOLDER") && updater_pubkey_is_valid(value)).then_some(value)
}

fn updater_pubkey_is_valid(value: &str) -> bool {
    STANDARD
        .decode(value.trim())
        .ok()
        .and_then(|decoded| String::from_utf8(decoded).ok())
        .and_then(|decoded| PublicKey::decode(&decoded).ok())
        .is_some()
}

fn configured_gitee_endpoint() -> Option<&'static str> {
    option_env!("TEMPLEFIX_GITEE_UPDATE_ENDPOINT")
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn endpoint_is_allowed(name: &str, endpoint: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(endpoint) else {
        return false;
    };
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    match name {
        "Gitee" => matches!(host.as_str(), "gitee.com" | "www.gitee.com"),
        "GitHub" => host == "github.com",
        _ => false,
    }
}

fn ordered_sources(locale: &str, gitee_endpoint: Option<&str>) -> Vec<UpdateSource> {
    let github = UpdateSource {
        name: "GitHub",
        endpoint: GITHUB_UPDATE_ENDPOINT.to_string(),
    };
    let gitee = gitee_endpoint
        .filter(|endpoint| endpoint_is_allowed("Gitee", endpoint))
        .map(|endpoint| UpdateSource {
            name: "Gitee",
            endpoint: endpoint.to_string(),
        });

    if locale == "zh-CN" {
        gitee.into_iter().chain([github]).collect()
    } else {
        [github].into_iter().chain(gitee).collect()
    }
}

fn base_status(app: &AppHandle) -> AppUpdateStatus {
    let cfg = config::load(app);
    let locale = i18n::locale(&cfg);
    let sources = ordered_sources(locale, configured_gitee_endpoint());
    AppUpdateStatus {
        configured: release_pubkey().is_some() && !sources.is_empty(),
        gitee_configured: sources.iter().any(|source| source.name == "Gitee"),
        current_version: app.package_info().version.to_string(),
        checking: false,
        available: false,
        version: None,
        notes: None,
        published_at: None,
        source: None,
        last_checked_at: cfg.last_update_check_at,
        error: None,
    }
}

pub fn current_status(app: &AppHandle) -> AppUpdateStatus {
    let state = app.state::<AppUpdateState>();
    let mut status = state
        .last_status
        .lock()
        .ok()
        .and_then(|status| status.clone())
        .unwrap_or_else(|| base_status(app));
    let cfg = config::load(app);
    status.current_version = app.package_info().version.to_string();
    status.last_checked_at = cfg.last_update_check_at;
    status.checking = state.checking.load(Ordering::Acquire);
    status.configured = release_pubkey().is_some();
    status.gitee_configured = ordered_sources(i18n::locale(&cfg), configured_gitee_endpoint())
        .iter()
        .any(|source| source.name == "Gitee");
    status
}

fn version_of(update: &Update) -> Version {
    Version::parse(update.version.trim_start_matches('v')).unwrap_or_else(|_| Version::new(0, 0, 0))
}

async fn check_source(
    app: &AppHandle,
    source: &UpdateSource,
    pubkey: &str,
) -> Result<Option<Update>, String> {
    let endpoint = source
        .endpoint
        .parse()
        .map_err(|_| format!("{} 更新地址无效", source.name))?;
    let updater = app
        .updater_builder()
        .pubkey(pubkey)
        .endpoints(vec![endpoint])
        .map_err(|error| error.to_string())?
        .timeout(Duration::from_secs(SOURCE_TIMEOUT_SECS))
        .build()
        .map_err(|error| error.to_string())?;
    updater.check().await.map_err(|error| error.to_string())
}

fn store_completed_check(app: &AppHandle, checked_at: u64) {
    let mut cfg = config::load(app);
    cfg.last_update_check_at = checked_at;
    if let Err(error) = config::save(app, &cfg) {
        eprintln!("无法保存应用更新检查时间：{error}");
    }
}

fn publish_status(app: &AppHandle, status: AppUpdateStatus, pending: Vec<PendingUpdate>) {
    let state = app.state::<AppUpdateState>();
    if let Ok(mut slot) = state.pending.lock() {
        *slot = pending;
    }
    if let Ok(mut slot) = state.last_status.lock() {
        *slot = Some(status.clone());
    }
    let _ = app.emit("app-update-status", &status);
    if let Err(error) = crate::apply_native_language(app, &config::load(app)) {
        eprintln!("无法刷新更新托盘提示：{error}");
    }
}

pub async fn check(app: &AppHandle) -> AppUpdateStatus {
    let state = app.state::<AppUpdateState>();
    if state
        .checking
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return current_status(app);
    }
    let _guard = CheckingGuard(&state.checking);

    let mut status = base_status(app);
    status.checking = true;
    let _ = app.emit("app-update-status", &status);

    let Some(pubkey) = release_pubkey() else {
        status.checking = false;
        status.error = Some("正式更新签名尚未配置，本地构建不会联网检查更新".into());
        publish_status(app, status.clone(), Vec::new());
        return status;
    };

    let cfg = config::load(app);
    let sources = ordered_sources(i18n::locale(&cfg), configured_gitee_endpoint());
    let mut candidates = Vec::new();
    let mut completed_sources = 0usize;
    let mut errors = Vec::new();
    for (rank, source) in sources.into_iter().enumerate() {
        match check_source(app, &source, pubkey).await {
            Ok(Some(update)) => {
                completed_sources += 1;
                candidates.push(PendingUpdate {
                    update,
                    source,
                    rank,
                });
            }
            Ok(None) => completed_sources += 1,
            Err(error) => errors.push(format!("{}：{}", source.name, error)),
        }
    }

    candidates.sort_by(|left, right| {
        version_of(&right.update)
            .cmp(&version_of(&left.update))
            .then_with(|| left.rank.cmp(&right.rank))
    });

    status.checking = false;
    if completed_sources > 0 {
        let checked_at = now_unix();
        store_completed_check(app, checked_at);
        status.last_checked_at = checked_at;
    }

    if let Some(latest) = candidates.first() {
        let selected_version = latest.update.version.clone();
        let selected_notes = latest.update.body.clone();
        let selected_date = latest.update.date.map(|value| value.to_string());
        let selected_source = latest.source.name.to_string();
        candidates.retain(|item| item.update.version == selected_version);
        status.available = true;
        status.version = Some(selected_version);
        status.notes = selected_notes;
        status.published_at = selected_date;
        status.source = Some(selected_source);
    } else if completed_sources == 0 {
        status.error = Some(if errors.is_empty() {
            "没有可用的更新来源".into()
        } else {
            errors.join("；")
        });
    }

    publish_status(app, status.clone(), candidates);
    status
}

fn emit_progress(
    app: &AppHandle,
    stage: &'static str,
    source: &str,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
) {
    let percent = total_bytes
        .filter(|total| *total > 0)
        .map(|total| ((downloaded_bytes.saturating_mul(100) / total).min(100)) as u8)
        .unwrap_or(0);
    let _ = app.emit(
        "app-update-progress",
        UpdateProgress {
            stage,
            source: source.to_string(),
            downloaded_bytes,
            total_bytes,
            percent,
        },
    );
}

async fn download_verified(app: &AppHandle, pending: &PendingUpdate) -> Result<Vec<u8>, String> {
    let mut downloaded = 0u64;
    let mut last_percent = 0u8;
    let source = pending.source.name.to_string();
    emit_progress(app, "downloading", &source, 0, None);
    let progress_app = app.clone();
    let finish_app = app.clone();
    let progress_source = source.clone();
    let finish_source = source.clone();
    pending
        .update
        .download(
            move |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                let percent = total
                    .filter(|value| *value > 0)
                    .map(|value| ((downloaded.saturating_mul(100) / value).min(100)) as u8)
                    .unwrap_or(0);
                if percent != last_percent || total.is_none() {
                    last_percent = percent;
                    emit_progress(
                        &progress_app,
                        "downloading",
                        &progress_source,
                        downloaded,
                        total,
                    );
                }
            },
            move || emit_progress(&finish_app, "verifying", &finish_source, 0, None),
        )
        .await
        .map_err(|error| error.to_string())
}

pub async fn install(app: &AppHandle) -> Result<(), String> {
    let pending = app
        .state::<AppUpdateState>()
        .pending
        .lock()
        .map_err(|_| "更新状态被占用，请稍后重试".to_string())?
        .clone();
    if pending.is_empty() {
        return Err("请先检查更新".into());
    }

    let mut errors = Vec::new();
    for candidate in pending {
        match download_verified(app, &candidate).await {
            Ok(bytes) => {
                emit_progress(app, "installing", candidate.source.name, 0, None);
                candidate
                    .update
                    .install(bytes)
                    .map_err(|error| error.to_string())?;
                app.restart()
            }
            Err(error) => errors.push(format!("{}：{}", candidate.source.name, error)),
        }
    }
    Err(errors.join("；"))
}

pub fn schedule_auto_check(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(AUTO_CHECK_DELAY_SECS)).await;
        let cfg = config::load(&app);
        if cfg.auto_check_updates && is_auto_check_due(cfg.last_update_check_at, now_unix()) {
            let _ = check(&app).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::{
        endpoint_is_allowed, is_auto_check_due, ordered_sources, updater_pubkey_is_valid,
        CHECK_INTERVAL_SECS,
    };
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    #[test]
    fn simplified_chinese_prefers_gitee_and_other_locales_prefer_github() {
        let endpoint = "https://gitee.com/example/templefix/raw/main/latest.json";
        let zh = ordered_sources("zh-CN", Some(endpoint));
        let en = ordered_sources("en", Some(endpoint));
        assert_eq!(
            zh.iter().map(|item| item.name).collect::<Vec<_>>(),
            ["Gitee", "GitHub"]
        );
        assert_eq!(
            en.iter().map(|item| item.name).collect::<Vec<_>>(),
            ["GitHub", "Gitee"]
        );
    }

    #[test]
    fn update_endpoints_must_be_public_https_urls_on_expected_hosts() {
        assert!(endpoint_is_allowed(
            "Gitee",
            "https://gitee.com/example/templefix/raw/main/latest.json"
        ));
        assert!(endpoint_is_allowed(
            "GitHub",
            "https://github.com/example/templefix/releases/latest/download/latest.json"
        ));
        assert!(!endpoint_is_allowed(
            "Gitee",
            "http://gitee.com/latest.json"
        ));
        assert!(!endpoint_is_allowed(
            "Gitee",
            "https://gitee.com/latest.json?access_token=secret"
        ));
        assert!(!endpoint_is_allowed(
            "GitHub",
            "https://example.com/latest.json"
        ));
    }

    #[test]
    fn automatic_checks_are_limited_to_once_per_day() {
        let now = 2 * CHECK_INTERVAL_SECS;
        assert!(is_auto_check_due(0, now));
        assert!(!is_auto_check_due(now - CHECK_INTERVAL_SECS + 1, now));
        assert!(is_auto_check_due(now - CHECK_INTERVAL_SECS, now));
    }

    #[test]
    fn updater_public_key_must_use_the_tauri_encoded_minisign_format() {
        let minisign_key = "untrusted comment: minisign public key E7620F1842B4E81F\n\
                            RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
        let encoded = STANDARD.encode(minisign_key);
        assert!(updater_pubkey_is_valid(&encoded));
        assert!(!updater_pubkey_is_valid(minisign_key));
        assert!(!updater_pubkey_is_valid("not-a-public-key"));
    }
}
