//! RapidOCR optional component.
//!
//! The main MSI never contains the OCR runtime. Users can install a separate
//! package into the per-user application data directory. A persistent local
//! worker keeps the models warm and returns text plus confidence scores.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Mutex,
};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

const MANIFEST_NAME: &str = "manifest.json";
const COMPONENT_FOLDER: &str = "rapidocr";
const MIN_TEXT_SCORE: f64 = 0.60;
const WORKER_TIMEOUT: Duration = Duration::from_secs(45);
const MAX_COMPONENT_BYTES: u64 = 700 * 1024 * 1024;
const MAX_DOWNLOAD_OVERAGE: u64 = 1024 * 1024;
const DOWNLOAD_USER_AGENT: &str = "TempleFix/0.1 RapidOCR-Installer";
const RELEASE_CONFIG: &str = include_str!("../rapidocr-release.json");

#[derive(Default)]
pub struct RapidOcrState {
    worker: Mutex<Option<Worker>>,
    installing: AtomicBool,
}

impl RapidOcrState {
    pub fn stop(&self) {
        let mut worker = self.worker.lock().unwrap();
        worker.take();
    }

    fn begin_install(&self) -> Result<(), String> {
        self.installing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map(|_| ())
            .map_err(|_| "增强 OCR 正在安装，请稍候".to_string())
    }

    fn finish_install(&self) {
        self.installing.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RapidOcrStatus {
    pub installed: bool,
    pub installing: bool,
    pub version: String,
    pub engine: String,
    pub size_bytes: u64,
    pub supported_languages: Vec<String>,
    pub error: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ReleaseConfig {
    schema: u32,
    version: String,
    package_name: String,
    package_bytes: u64,
    installed_bytes: u64,
    sha256: String,
    sources: Vec<DownloadSource>,
}

#[derive(Debug, Clone, Deserialize)]
struct DownloadSource {
    name: String,
    url: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RapidOcrReleaseInfo {
    pub available: bool,
    pub version: String,
    pub package_bytes: u64,
    pub installed_bytes: u64,
    pub source_names: Vec<String>,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallProgress {
    pub stage: String,
    pub percent: u8,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub message: String,
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct ComponentManifest {
    schema: u32,
    name: String,
    version: String,
    engine: String,
    worker: String,
    supported_languages: Vec<String>,
    checksums: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct WorkerResponse {
    id: Option<u64>,
    ok: bool,
    error: Option<String>,
    #[serde(default)]
    elapsed_ms: u64,
    #[serde(default)]
    lines: Vec<WorkerLine>,
}

#[derive(Debug, Deserialize)]
struct WorkerLine {
    text: String,
    score: f64,
}

struct Worker {
    child: Child,
    stdin: ChildStdin,
    responses: mpsc::Receiver<String>,
    next_id: u64,
}

impl Worker {
    fn start(executable: &Path) -> Result<Self, String> {
        let mut command = Command::new(executable);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("启动增强 OCR 失败：{e}"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "增强 OCR 输入管道创建失败".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "增强 OCR 输出管道创建失败".to_string())?;
        let (sender, responses) = mpsc::channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            stdin,
            responses,
            next_id: 1,
        })
    }

    fn request(&mut self, image_bytes: &[u8]) -> Result<WorkerResponse, String> {
        if let Some(status) = self
            .child
            .try_wait()
            .map_err(|e| format!("检查增强 OCR 状态失败：{e}"))?
        {
            return Err(format!("增强 OCR 已异常退出：{status}"));
        }

        let id = self.next_id;
        self.next_id += 1;
        let request = serde_json::json!({
            "id": id,
            "image_base64": base64::engine::general_purpose::STANDARD.encode(image_bytes),
        });
        serde_json::to_writer(&mut self.stdin, &request)
            .map_err(|e| format!("发送图片到增强 OCR 失败：{e}"))?;
        self.stdin
            .write_all(b"\n")
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("发送图片到增强 OCR 失败：{e}"))?;

        let raw = self
            .responses
            .recv_timeout(WORKER_TIMEOUT)
            .map_err(|e| match e {
                mpsc::RecvTimeoutError::Timeout => "增强 OCR 处理超时".to_string(),
                mpsc::RecvTimeoutError::Disconnected => "增强 OCR 连接已断开".to_string(),
            })?;
        let response: WorkerResponse =
            serde_json::from_str(&raw).map_err(|e| format!("增强 OCR 返回格式异常：{e}"))?;
        if response.id != Some(id) {
            return Err("增强 OCR 返回了错误的请求编号".into());
        }
        if !response.ok {
            return Err(response.error.unwrap_or_else(|| "增强 OCR 识别失败".into()));
        }
        Ok(response)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn component_parent(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|path| path.join("components"))
        .map_err(|e| format!("无法确定组件目录：{e}"))
}

fn component_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(component_parent(app)?.join(COMPONENT_FOLDER))
}

fn load_release_config() -> Result<ReleaseConfig, String> {
    let config: ReleaseConfig = serde_json::from_str(RELEASE_CONFIG)
        .map_err(|e| format!("增强 OCR 发布信息格式错误：{e}"))?;
    if config.schema != 1
        || config.version.trim().is_empty()
        || config.package_name.trim().is_empty()
        || config.package_bytes == 0
        || config.installed_bytes == 0
        || config.sha256.len() != 64
    {
        return Err("增强 OCR 发布信息不完整".into());
    }
    Ok(config)
}

fn configured_sources(config: &ReleaseConfig) -> Vec<DownloadSource> {
    let mut sources = config.sources.clone();
    if let Some(url) = option_env!("TEMPLEFIX_RAPIDOCR_MODELSCOPE_URL") {
        if !url.trim().is_empty() {
            sources.insert(
                0,
                DownloadSource {
                    name: "魔搭 ModelScope（中国大陆）".into(),
                    url: url.trim().into(),
                },
            );
        }
    }
    if let Some(url) = option_env!("TEMPLEFIX_RAPIDOCR_GITHUB_URL") {
        if !url.trim().is_empty() {
            sources.push(DownloadSource {
                name: "GitHub（备用）".into(),
                url: url.trim().into(),
            });
        }
    }
    #[cfg(debug_assertions)]
    if let Ok(url) = std::env::var("TEMPLEFIX_RAPIDOCR_DOWNLOAD_URL") {
        if !url.trim().is_empty() {
            sources.insert(
                0,
                DownloadSource {
                    name: "本机测试源".into(),
                    url: url.trim().into(),
                },
            );
        }
    }
    sources.retain(|source| !source.name.trim().is_empty() && !source.url.trim().is_empty());
    sources
}

fn source_url_is_allowed(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return false;
    };
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    if parsed.scheme() == "https"
        && matches!(
            host.as_str(),
            "modelscope.cn" | "www.modelscope.cn" | "github.com"
        )
    {
        return true;
    }
    #[cfg(debug_assertions)]
    if parsed.scheme() == "http" && matches!(host.as_str(), "127.0.0.1" | "localhost") {
        return true;
    }
    false
}

pub fn release_info() -> RapidOcrReleaseInfo {
    match load_release_config() {
        Ok(config) => {
            let sources: Vec<_> = configured_sources(&config)
                .into_iter()
                .filter(|source| source_url_is_allowed(&source.url))
                .collect();
            RapidOcrReleaseInfo {
                available: !sources.is_empty(),
                version: config.version,
                package_bytes: config.package_bytes,
                installed_bytes: config.installed_bytes,
                source_names: sources.into_iter().map(|source| source.name).collect(),
                error: String::new(),
            }
        }
        Err(error) => RapidOcrReleaseInfo {
            error,
            ..Default::default()
        },
    }
}

fn emit_progress(
    app: &AppHandle,
    stage: &str,
    percent: u8,
    downloaded_bytes: u64,
    total_bytes: u64,
    message: impl Into<String>,
    source: impl Into<String>,
) {
    let _ = app.emit_to(
        "settings",
        "rapidocr-install-progress",
        InstallProgress {
            stage: stage.into(),
            percent: percent.min(100),
            downloaded_bytes,
            total_bytes,
            message: message.into(),
            source: source.into(),
        },
    );
}

fn is_safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path.components().all(|part| {
            matches!(part, Component::Normal(_) | Component::CurDir)
        })
}

fn load_manifest(directory: &Path) -> Result<ComponentManifest, String> {
    let bytes = fs::read(directory.join(MANIFEST_NAME))
        .map_err(|e| format!("读取增强 OCR 清单失败：{e}"))?;
    let json = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let manifest: ComponentManifest =
        serde_json::from_slice(json).map_err(|e| format!("增强 OCR 清单格式错误：{e}"))?;
    if manifest.schema != 1 || manifest.name != "TempleFix RapidOCR" {
        return Err("不是有效的 TempleFix RapidOCR 组件包".into());
    }
    let worker = Path::new(&manifest.worker);
    if !is_safe_relative(worker) || !directory.join(worker).is_file() {
        return Err("增强 OCR 主程序缺失".into());
    }
    Ok(manifest)
}

fn directory_size(directory: &Path) -> u64 {
    fn visit(directory: &Path, total: &mut u64) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                visit(&entry.path(), total);
            } else if kind.is_file() {
                *total = total.saturating_add(entry.metadata().map(|m| m.len()).unwrap_or(0));
            }
        }
    }
    let mut total = 0;
    visit(directory, &mut total);
    total
}

pub fn status(app: &AppHandle) -> RapidOcrStatus {
    let installing = app
        .try_state::<RapidOcrState>()
        .map(|state| state.installing.load(Ordering::SeqCst))
        .unwrap_or(false);
    let directory = match component_dir(app) {
        Ok(path) => path,
        Err(error) => {
            return RapidOcrStatus {
                installing,
                error,
                ..Default::default()
            }
        }
    };
    if !directory.exists() {
        return RapidOcrStatus {
            installing,
            ..Default::default()
        };
    }
    match load_manifest(&directory) {
        Ok(manifest) => RapidOcrStatus {
            installed: true,
            installing,
            version: manifest.version,
            engine: manifest.engine,
            size_bytes: directory_size(&directory),
            supported_languages: manifest.supported_languages,
            error: String::new(),
        },
        Err(error) => RapidOcrStatus {
            installing,
            size_bytes: directory_size(&directory),
            error,
            ..Default::default()
        },
    }
}

pub fn supports_language(app: &AppHandle, source_lang: &str) -> bool {
    if source_lang.trim().is_empty() {
        return status(app).installed;
    }
    let normalized = match source_lang.trim() {
        "简体中文" => "Chinese (Simplified)",
        "繁體中文" => "Chinese (Traditional)",
        "日本語" => "Japanese",
        "Français" => "French",
        "Português" => "Portuguese",
        "Español" => "Spanish",
        other => other,
    };
    let current = status(app);
    current.installed
        && current
            .supported_languages
            .iter()
            .any(|language| language.eq_ignore_ascii_case(normalized))
}

fn worker_executable(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = component_dir(app)?;
    let manifest = load_manifest(&directory)?;
    Ok(directory.join(manifest.worker))
}

fn filter_lines(lines: &[WorkerLine]) -> (String, usize) {
    let mut accepted = Vec::new();
    let mut rejected = 0;
    for line in lines {
        let text = line.text.trim();
        if text.is_empty() {
            continue;
        }
        if line.score >= MIN_TEXT_SCORE {
            accepted.push(text.to_string());
        } else {
            rejected += 1;
        }
    }
    (accepted.join("\n"), rejected)
}

pub fn recognize(
    app: &AppHandle,
    image_bytes: &[u8],
) -> Result<String, String> {
    let executable = worker_executable(app)?;
    let state = app.state::<RapidOcrState>();
    let mut slot = state.worker.lock().unwrap();

    for attempt in 0..2 {
        if slot.is_none() {
            *slot = Some(Worker::start(&executable)?);
        }
        let result = slot.as_mut().unwrap().request(image_bytes);
        match result {
            Ok(response) => {
                let (text, rejected) = filter_lines(&response.lines);
                crate::log_debug(&format!(
                    "RapidOCR: 行数={} 过滤={} 耗时={}ms",
                    response.lines.len(),
                    rejected,
                    response.elapsed_ms
                ));
                if text.trim().is_empty() {
                    return Err("增强 OCR 没有识别到可靠文字".into());
                }
                return Ok(text);
            }
            Err(error) if attempt == 0 => {
                crate::log_debug(&format!("RapidOCR 首次调用失败，重启组件：{error}"));
                slot.take();
            }
            Err(error) => return Err(error),
        }
    }
    Err("增强 OCR 识别失败".into())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("读取组件文件失败：{e}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|e| format!("读取组件文件失败：{e}"))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

async fn download_one<F>(
    client: &reqwest::Client,
    source: &DownloadSource,
    expected_bytes: u64,
    expected_sha256: &str,
    target: &Path,
    mut on_chunk: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64) + Send,
{
    if !source_url_is_allowed(&source.url) {
        return Err(format!("下载地址不受信任：{}", source.name));
    }
    let mut response = client
        .get(&source.url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await
        .map_err(|e| format!("连接{}失败：{e}", source.name))?;
    if !response.status().is_success() {
        return Err(format!(
            "{}返回 HTTP {}",
            source.name,
            response.status().as_u16()
        ));
    }
    if let Some(length) = response.content_length() {
        if length > expected_bytes.saturating_add(MAX_DOWNLOAD_OVERAGE) {
            return Err(format!("{}返回的文件体积异常", source.name));
        }
    }

    let mut output = tokio::fs::File::create(target)
        .await
        .map_err(|e| format!("创建组件下载文件失败：{e}"))?;
    let mut hash = Sha256::new();
    let mut downloaded = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("从{}下载时连接中断：{e}", source.name))?
    {
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if downloaded > expected_bytes.saturating_add(MAX_DOWNLOAD_OVERAGE) {
            return Err(format!("{}返回的文件体积异常", source.name));
        }
        output
            .write_all(&chunk)
            .await
            .map_err(|e| format!("保存组件下载文件失败：{e}"))?;
        hash.update(&chunk);
        on_chunk(downloaded, expected_bytes);
    }
    output
        .flush()
        .await
        .map_err(|e| format!("保存组件下载文件失败：{e}"))?;
    drop(output);

    if downloaded != expected_bytes {
        return Err(format!(
            "{}下载不完整：应为 {} 字节，实际 {} 字节",
            source.name, expected_bytes, downloaded
        ));
    }
    let actual = format!("{:x}", hash.finalize());
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        return Err(format!("{}下载文件校验失败", source.name));
    }
    Ok(())
}

fn build_download_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(20 * 60))
        .user_agent(DOWNLOAD_USER_AGENT)
        .build()
        .map_err(|e| format!("创建组件下载连接失败：{e}"))
}

async fn download_package(app: &AppHandle, config: &ReleaseConfig) -> Result<(PathBuf, String), String> {
    let sources: Vec<_> = configured_sources(config)
        .into_iter()
        .filter(|source| source_url_is_allowed(&source.url))
        .collect();
    if sources.is_empty() {
        return Err("自动下载源尚未发布；可以暂时使用“从本地 ZIP 安装”".into());
    }
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("无法确定下载缓存目录：{e}"))?
        .join("component-downloads");
    tokio::fs::create_dir_all(&cache)
        .await
        .map_err(|e| format!("创建下载缓存目录失败：{e}"))?;
    let target = cache.join(format!("rapidocr-{}-{}.zip", config.version, std::process::id()));
    let client = build_download_client()?;

    let mut errors = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let _ = tokio::fs::remove_file(&target).await;
        emit_progress(
            app,
            "connecting",
            2,
            0,
            config.package_bytes,
            if index == 0 {
                format!("正在连接{}…", source.name)
            } else {
                format!("正在尝试备用源：{}…", source.name)
            },
            source.name.clone(),
        );
        let progress_app = app.clone();
        let source_name = source.name.clone();
        let result = download_one(
            &client,
            source,
            config.package_bytes,
            &config.sha256,
            &target,
            move |downloaded, total| {
                let ratio = if total == 0 {
                    0.0
                } else {
                    downloaded as f64 / total as f64
                };
                emit_progress(
                    &progress_app,
                    "downloading",
                    (5.0 + ratio * 65.0).round() as u8,
                    downloaded,
                    total,
                    "正在下载增强 OCR…",
                    source_name.clone(),
                );
            },
        )
        .await;
        match result {
            Ok(()) => return Ok((target, source.name.clone())),
            Err(error) => {
                crate::log_debug(&format!("RapidOCR 下载源失败：{error}"));
                errors.push(error);
            }
        }
    }
    let _ = tokio::fs::remove_file(&target).await;
    Err(format!("所有下载源均失败：{}", errors.join("；")))
}

fn verify_package(directory: &Path, manifest: &ComponentManifest) -> Result<(), String> {
    if manifest.checksums.is_empty() || !manifest.checksums.contains_key(&manifest.worker) {
        return Err("组件包没有完整性校验信息".into());
    }
    for (relative, expected) in &manifest.checksums {
        let relative_path = Path::new(relative);
        if !is_safe_relative(relative_path) {
            return Err("组件清单包含不安全路径".into());
        }
        let actual = sha256_file(&directory.join(relative_path))?;
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(format!("组件文件校验失败：{relative}"));
        }
    }
    Ok(())
}

fn extract_package_with_progress<F>(
    package_path: &Path,
    staging: &Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(u64, u64),
{
    let package = File::open(package_path).map_err(|e| format!("打开组件包失败：{e}"))?;
    let mut archive =
        zip::ZipArchive::new(package).map_err(|e| format!("组件包损坏：{e}"))?;
    let mut total = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|e| format!("读取组件包失败：{e}"))?;
        total = total.saturating_add(entry.size());
        if total > MAX_COMPONENT_BYTES {
            return Err("组件包解压后体积异常".into());
        }
    }

    let mut extracted = 0u64;
    let mut buffer = vec![0u8; 256 * 1024];
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("读取组件包失败：{e}"))?;
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| "组件包包含不安全路径".to_string())?
            .to_path_buf();
        if !is_safe_relative(&relative) {
            return Err("组件包包含不安全路径".into());
        }
        let output = staging.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|e| format!("创建组件子目录失败：{e}"))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("创建组件子目录失败：{e}"))?;
        }
        let mut output_file =
            File::create(&output).map_err(|e| format!("写入组件失败：{e}"))?;
        loop {
            let count = entry
                .read(&mut buffer)
                .map_err(|e| format!("读取组件压缩数据失败：{e}"))?;
            if count == 0 {
                break;
            }
            output_file
                .write_all(&buffer[..count])
                .map_err(|e| format!("写入组件失败：{e}"))?;
            extracted = extracted.saturating_add(count as u64);
            on_progress(extracted, total);
        }
    }
    let manifest = load_manifest(staging)?;
    verify_package(staging, &manifest)
}

#[cfg(test)]
fn extract_package(package_path: &Path, staging: &Path) -> Result<(), String> {
    extract_package_with_progress(package_path, staging, |_, _| {})
}

fn install_package_with_progress<F>(
    app: &AppHandle,
    package_path: &Path,
    on_progress: F,
) -> Result<RapidOcrStatus, String>
where
    F: FnMut(u64, u64),
{
    let parent = component_parent(app)?;
    fs::create_dir_all(&parent).map_err(|e| format!("创建组件目录失败：{e}"))?;
    let staging = parent.join(format!(".rapidocr-install-{}", std::process::id()));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|e| format!("清理临时组件目录失败：{e}"))?;
    }
    fs::create_dir_all(&staging).map_err(|e| format!("创建临时组件目录失败：{e}"))?;

    let extraction = extract_package_with_progress(package_path, &staging, on_progress);
    if let Err(error) = extraction {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }

    app.state::<RapidOcrState>().stop();
    let destination = component_dir(app)?;
    let backup = parent.join(".rapidocr-backup");
    if backup.exists() {
        fs::remove_dir_all(&backup).map_err(|e| format!("清理旧组件备份失败：{e}"))?;
    }
    if destination.exists() {
        fs::rename(&destination, &backup).map_err(|e| format!("备份旧组件失败：{e}"))?;
    }
    if let Err(error) = fs::rename(&staging, &destination) {
        if backup.exists() {
            let _ = fs::rename(&backup, &destination);
        }
        return Err(format!("安装增强 OCR 失败：{error}"));
    }
    if backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }
    Ok(status(app))
}

pub async fn download_and_install(app: &AppHandle) -> Result<RapidOcrStatus, String> {
    app.state::<RapidOcrState>().begin_install()?;
    let result = async {
        let config = load_release_config()?;
        let (package, source) = download_package(app, &config).await?;
        emit_progress(
            app,
            "verifying",
            72,
            config.package_bytes,
            config.package_bytes,
            "下载完成，完整性校验通过",
            source.clone(),
        );
        let worker_app = app.clone();
        let progress_app = app.clone();
        let progress_source = source.clone();
        let install_path = package.clone();
        let installed = match tokio::task::spawn_blocking(move || {
            install_package_with_progress(&worker_app, &install_path, move |done, total| {
                let ratio = if total == 0 {
                    0.0
                } else {
                    done as f64 / total as f64
                };
                emit_progress(
                    &progress_app,
                    "installing",
                    (75.0 + ratio * 23.0).round() as u8,
                    done,
                    total,
                    "正在安装增强 OCR…",
                    progress_source.clone(),
                );
            })
        })
        .await
        {
            Ok(result) => result,
            Err(error) => Err(format!("安装任务失败：{error}")),
        };
        let _ = tokio::fs::remove_file(&package).await;
        installed?;
        emit_progress(
            app,
            "success",
            100,
            config.package_bytes,
            config.package_bytes,
            "增强 OCR 安装成功并已启用",
            source,
        );
        Ok::<(), String>(())
    }
    .await;
    app.state::<RapidOcrState>().finish_install();
    match result {
        Ok(()) => Ok(status(app)),
        Err(error) => {
            emit_progress(
                app,
                "failed",
                0,
                0,
                0,
                format!("安装失败：{error}"),
                "",
            );
            Err(error)
        }
    }
}

pub async fn install_local_package_with_events(
    app: &AppHandle,
    package_path: PathBuf,
) -> Result<RapidOcrStatus, String> {
    app.state::<RapidOcrState>().begin_install()?;
    let config = match load_release_config() {
        Ok(config) => config,
        Err(error) => {
            app.state::<RapidOcrState>().finish_install();
            return Err(error);
        }
    };
    emit_progress(
        app,
        "verifying",
        5,
        0,
        config.package_bytes,
        "正在校验本地组件包…",
        "本地 ZIP",
    );
    let worker_app = app.clone();
    let progress_app = app.clone();
    let expected_bytes = config.package_bytes;
    let expected_sha256 = config.sha256.clone();
    let result = match tokio::task::spawn_blocking(move || {
        let actual_bytes = fs::metadata(&package_path)
            .map_err(|e| format!("读取本地组件包失败：{e}"))?
            .len();
        if actual_bytes != expected_bytes {
            return Err("所选 ZIP 不是当前版本的官方组件包（文件体积不符）".into());
        }
        let actual_sha256 = sha256_file(&package_path)?;
        if !actual_sha256.eq_ignore_ascii_case(&expected_sha256) {
            return Err("所选 ZIP 不是当前版本的官方组件包（校验值不符）".into());
        }
        emit_progress(
            &progress_app,
            "verifying",
            15,
            expected_bytes,
            expected_bytes,
            "组件包校验通过",
            "本地 ZIP",
        );
        install_package_with_progress(&worker_app, &package_path, move |done, total| {
            let ratio = if total == 0 {
                0.0
            } else {
                done as f64 / total as f64
            };
            emit_progress(
                &progress_app,
                "installing",
                (18.0 + ratio * 80.0).round() as u8,
                done,
                total,
                "正在安装增强 OCR…",
                "本地 ZIP",
            );
        })
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("安装任务失败：{error}")),
    };
    app.state::<RapidOcrState>().finish_install();
    match result {
        Ok(_) => {
            emit_progress(
                app,
                "success",
                100,
                config.package_bytes,
                config.package_bytes,
                "增强 OCR 安装成功并已启用",
                "本地 ZIP",
            );
            Ok(status(app))
        }
        Err(error) => {
            emit_progress(
                app,
                "failed",
                0,
                0,
                0,
                format!("安装失败：{error}"),
                "本地 ZIP",
            );
            Err(error)
        }
    }
}

pub fn uninstall(app: &AppHandle) -> Result<RapidOcrStatus, String> {
    if app
        .state::<RapidOcrState>()
        .installing
        .load(Ordering::SeqCst)
    {
        return Err("增强 OCR 正在安装，暂时不能卸载".into());
    }
    app.state::<RapidOcrState>().stop();
    let directory = component_dir(app)?;
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(|e| format!("卸载增强 OCR 失败：{e}"))?;
    }
    Ok(status(app))
}

#[cfg(test)]
mod tests {
    use super::{
        build_download_client, download_one, extract_package, filter_lines, is_safe_relative, load_manifest,
        load_release_config, source_url_is_allowed, verify_package, DownloadSource, Worker,
        WorkerLine,
    };
    use std::path::Path;

    #[test]
    fn rejects_component_paths_that_can_escape_install_directory() {
        assert!(is_safe_relative(Path::new("rapidocr_worker.exe")));
        assert!(is_safe_relative(Path::new("_internal/models/model.onnx")));
        assert!(!is_safe_relative(Path::new("../outside.exe")));
        assert!(!is_safe_relative(Path::new("C:\\outside.exe")));
    }

    #[test]
    fn low_confidence_ocr_lines_are_not_sent_to_translation() {
        let lines = vec![
            WorkerLine {
                text: "SPIDER-MAN".into(),
                score: 0.999,
            },
            WorkerLine {
                text: "MLSIE".into(),
                score: 0.542,
            },
            WorkerLine {
                text: "No Way Hame".into(),
                score: 0.941,
            },
        ];
        let (text, rejected) = filter_lines(&lines);
        assert_eq!(text, "SPIDER-MAN\nNo Way Hame");
        assert_eq!(rejected, 1);
    }

    #[test]
    fn bundled_release_metadata_is_complete_and_pinned() {
        let release = load_release_config().expect("内置组件发布信息无效");
        assert_eq!(release.schema, 1);
        assert_eq!(release.version, "3.9.2-1");
        assert_eq!(release.package_name, "TempleFix_RapidOCR_Addon_3.9.2-1.zip");
        assert_eq!(release.package_bytes, 97_445_269);
        assert_eq!(release.installed_bytes, 217_049_422);
        assert_eq!(
            release.sha256.to_ascii_uppercase(),
            "5D43C5BAF2A76434765FDFCF529921640653D6CFB56ADF35ADF4F39A138523D1"
        );
        assert_eq!(release.sources.len(), 1);
        assert_eq!(release.sources[0].name, "魔搭 ModelScope（中国大陆）");
        assert!(source_url_is_allowed(&release.sources[0].url));
    }

    #[test]
    fn component_downloads_only_use_trusted_hosts() {
        assert!(source_url_is_allowed(
            "https://www.modelscope.cn/models/example/templefix/resolve/main/addon.zip"
        ));
        assert!(source_url_is_allowed(
            "https://github.com/example/templefix/releases/download/v1/addon.zip"
        ));
        assert!(!source_url_is_allowed("http://modelscope.cn/addon.zip"));
        assert!(!source_url_is_allowed("https://example.com/addon.zip"));
        assert!(!source_url_is_allowed("file:///C:/addon.zip"));
    }

    #[tokio::test]
    async fn real_component_download_is_verified_when_requested() {
        let Ok(url) = std::env::var("TEMPLEFIX_RAPIDOCR_DOWNLOAD_TEST_URL") else {
            return;
        };
        let release = load_release_config().expect("内置组件发布信息无效");
        let source = DownloadSource {
            name: "本地测试源".into(),
            url,
        };
        let target = std::env::temp_dir().join(format!(
            "templefix-rapidocr-download-test-{}.zip",
            std::process::id()
        ));
        let client = build_download_client().expect("创建组件下载连接失败");
        let result = download_one(
            &client,
            &source,
            release.package_bytes,
            &release.sha256,
            &target,
            |_, _| {},
        )
        .await;
        let cleanup = tokio::fs::remove_file(&target).await;
        result.expect("真实组件下载或校验失败");
        cleanup.expect("清理下载测试文件失败");
    }

    #[test]
    fn packaged_component_is_valid_when_requested() {
        let Ok(directory) = std::env::var("TEMPLEFIX_RAPIDOCR_COMPONENT_DIR") else {
            return;
        };
        let directory = Path::new(&directory);
        let manifest = load_manifest(directory).expect("组件清单无效");
        verify_package(directory, &manifest).expect("组件文件校验失败");
    }

    #[test]
    fn packaged_zip_extracts_and_validates_when_requested() {
        let Ok(package) = std::env::var("TEMPLEFIX_RAPIDOCR_PACKAGE") else {
            return;
        };
        let target = std::env::temp_dir().join(format!(
            "templefix-rapidocr-package-test-{}",
            std::process::id()
        ));
        if target.exists() {
            std::fs::remove_dir_all(&target).expect("清理旧组件测试目录失败");
        }
        std::fs::create_dir_all(&target).expect("创建组件测试目录失败");
        let result = extract_package(Path::new(&package), &target);
        let cleanup = std::fs::remove_dir_all(&target);
        result.expect("真实组件 ZIP 解压或校验失败");
        cleanup.expect("清理组件测试目录失败");
    }

    #[test]
    fn packaged_worker_recognizes_local_probe_when_requested() {
        let Ok(worker_path) = std::env::var("TEMPLEFIX_RAPIDOCR_WORKER") else {
            return;
        };
        let Ok(image_path) = std::env::var("TEMPLEFIX_RAPIDOCR_TEST_IMAGE") else {
            return;
        };
        let bytes = std::fs::read(image_path).expect("读取 RapidOCR 探针图片失败");
        let mut worker = Worker::start(Path::new(&worker_path)).expect("启动 RapidOCR 失败");
        let response = worker.request(&bytes).expect("RapidOCR 请求失败");
        let (text, _) = filter_lines(&response.lines);
        assert!(text.contains("SPIDER-MAN"), "未识别出预期标题：{text}");
        assert!(!text.contains("MLSIE"), "低置信度商标噪声未被过滤：{text}");
    }
}
