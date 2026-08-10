/* ===== 设置页逻辑 ===== */
(function () {
  "use strict";

  let providers = [];
  let cfg = null;
  let rapidStatus = null;
  let rapidRelease = null;
  let initialized = false;
  let installMode = "auto";
  let installInFlight = false;
  let lastInstallStage = "downloading";

  const $ = (id) => document.getElementById(id);

  async function init() {
    const [config, provs, componentStatus, releaseInfo] = await Promise.all([
      TF.invoke("get_config"),
      TF.invoke("list_providers"),
      TF.invoke("rapidocr_status"),
      TF.invoke("rapidocr_release_info"),
    ]);
    cfg = config;
    providers = provs;
    rapidStatus = componentStatus;
    rapidRelease = releaseInfo;

    fillProviders("llm_provider");
    fillProviders("mm_provider");
    fillSourceLanguages(config.source_lang || "");
    fillForm(config);
    renderRapidOcr();

    if (initialized) return;
    initialized = true;

    $("llm_provider").addEventListener("change", () => {
      const p = providers.find((x) => x.id === $("llm_provider").value);
      if (p) $("llm_base_url").value = p.default_base_url;
    });
    $("mm_provider").addEventListener("change", () => {
      const p = providers.find((x) => x.id === $("mm_provider").value);
      if (p) $("mm_base_url").value = p.default_base_url;
    });
    $("use_multimodal").addEventListener("change", updateMmToggle);
    $("rapidocr_install").addEventListener("click", () => openInstallWizard("auto"));
    $("rapidocr_local_install").addEventListener("click", () => openInstallWizard("local"));
    $("rapidocr_uninstall").addEventListener("click", uninstallRapidOcr);
    $("rapidocr_modal_start").addEventListener("click", startInstall);
    $("rapidocr_modal_cancel").addEventListener("click", closeInstallWizard);
    $("rapidocr_modal").addEventListener("click", (event) => {
      if (event.target === $("rapidocr_modal") && !installInFlight) closeInstallWizard();
    });
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && !installInFlight) closeInstallWizard();
    });
    $("save").addEventListener("click", save);

    TF.listen("rapidocr-install-progress", (event) => updateInstallProgress(event.payload || {}));
  }

  function fillForm(config) {
    cfg = config;
    $("llm_provider").value = cfg.llm_provider || "custom";
    $("llm_base_url").value = cfg.llm_base_url || "";
    $("llm_model").value = cfg.llm_model || "";
    $("llm_api_key").value = cfg.llm_api_key || "";

    $("use_multimodal").checked = cfg.use_multimodal || false;
    $("mm_provider").value = cfg.mm_provider || "doubao";
    $("mm_base_url").value = cfg.mm_base_url || "";
    $("mm_model").value = cfg.mm_model || "";
    $("mm_api_key").value = cfg.mm_api_key || "";
    $("multimodal_threshold").value = cfg.multimodal_threshold || 5;

    $("native_lang").value = cfg.native_lang || "简体中文";
    $("target_lang").value = cfg.target_lang || "";
    $("source_lang").value = cfg.source_lang || "";
    $("use_rapidocr").checked = cfg.use_rapidocr !== false;
    $("hotkey").value = cfg.hotkey || "Alt+Z";

    updateMmToggle();
  }

  function fillProviders(selectId) {
    const sel = $(selectId);
    sel.innerHTML = "";
    for (const p of providers) {
      const opt = document.createElement("option");
      opt.value = p.id;
      opt.textContent = p.name;
      sel.appendChild(opt);
    }
  }

  function fillSourceLanguages(selected) {
    const displayAliases = {
      "Chinese (Simplified)": "简体中文",
      "Chinese (Traditional)": "繁體中文",
      Japanese: "日本語",
      French: "Français",
      Portuguese: "Português",
      Spanish: "Español",
    };
    const common = [
      "简体中文", "繁體中文", "English", "日本語", "한국어",
      "Français", "Português", "Español",
    ];
    const enhanced = rapidStatus && rapidStatus.installed
      ? rapidStatus.supported_languages || []
      : [];
    const languages = [
      ...new Set([...common, ...enhanced.map((name) => displayAliases[name] || name)]),
    ];
    const select = $("source_lang");
    select.innerHTML = '<option value="">自动识别</option>';
    for (const language of languages) {
      const option = document.createElement("option");
      option.value = language;
      option.textContent = language;
      select.appendChild(option);
    }
    select.value = displayAliases[selected] || selected;
  }

  function formatSize(bytes) {
    if (!bytes) return "—";
    return (bytes / 1024 / 1024).toFixed(1) + " MB";
  }

  function renderRapidOcr() {
    const status = rapidStatus || {};
    const busy = !!status.installing || installInFlight;
    const dot = $("rapidocr_dot");
    dot.classList.toggle("ready", !!status.installed);
    dot.classList.toggle("broken", !status.installed && !!status.error);
    $("use_rapidocr").disabled = !status.installed;
    $("rapidocr_uninstall").style.display =
      status.installed || status.error ? "" : "none";
    $("rapidocr_install").textContent = status.installed ? "重新安装" : "一键安装";
    $("rapidocr_install").disabled = busy;
    $("rapidocr_local_install").disabled = busy;
    $("rapidocr_uninstall").disabled = busy;
    $("rapidocr_busy").textContent = busy ? "正在安装…" : "";

    if (status.installed) {
      const size = formatSize(status.size_bytes);
      $("rapidocr_status").textContent =
        "已安装 " + status.version + (size !== "—" ? " · " + size : "") + " · 可完全离线识别";
      $("source_lang_desc").textContent =
        "增强 OCR 已启用，支持 " + status.supported_languages.length + " 种原文语言；其他语言使用 Windows OCR";
    } else if (status.error) {
      $("rapidocr_status").textContent = "组件异常：" + status.error;
      $("source_lang_desc").textContent =
        "增强 OCR 组件异常，当前使用 Windows 自带 OCR";
    } else {
      $("rapidocr_status").textContent =
        "未安装 · 主程序仍保持轻量，当前使用 Windows 自带 OCR";
      $("source_lang_desc").textContent =
        "安装增强组件后可使用中、英、日及多种拉丁文字";
    }
  }

  function openInstallWizard(mode) {
    if (installInFlight || (rapidStatus && rapidStatus.installing)) return;
    installMode = mode;
    lastInstallStage = mode === "auto" ? "downloading" : "verifying";
    const release = rapidRelease || {};
    const sourceNames = release.source_names || [];
    const source = mode === "local" ? "你选择的本地 ZIP" : sourceNames.join("、");
    const isUpdate = rapidStatus && rapidStatus.installed;
    const automaticAvailable = !!release.available;

    $("rapidocr_modal").classList.add("open");
    $("rapidocr_modal").setAttribute("aria-hidden", "false");
    $("rapidocr_modal_icon").textContent = "↓";
    $("rapidocr_modal_icon").className = "install-hero";
    $("rapidocr_modal_title").textContent = isUpdate ? "更新增强离线 OCR" : "安装增强离线 OCR";
    $("rapidocr_modal_summary").textContent = mode === "local"
      ? "下一步会让你选择官方组件 ZIP，随后自动校验和安装。"
      : automaticAvailable
        ? "确认后会自动下载、校验并安装。每一步和安装结果都会显示在这里。"
        : "自动下载源尚未发布。你仍可关闭此窗口，使用下方的本地 ZIP 高级入口。";
    $("rapidocr_download_size").textContent = formatSize(release.package_bytes);
    $("rapidocr_installed_size").textContent = formatSize(release.installed_bytes);
    $("rapidocr_source_name").textContent = source || "尚未发布";
    $("rapidocr_source_name").title = source || "尚未发布";
    $("rapidocr_progress_wrap").classList.remove("visible");
    $("rapidocr_progress_bar").style.width = "0%";
    $("rapidocr_progress_percent").textContent = "0%";
    $("rapidocr_progress_detail").textContent = "准备中…";
    $("rapidocr_modal_error").classList.toggle("visible", mode === "auto" && !automaticAvailable);
    $("rapidocr_modal_error").textContent = mode === "auto" && !automaticAvailable
      ? (release.error || "自动下载源尚未发布。")
      : "";
    resetInstallSteps();

    $("rapidocr_modal_cancel").disabled = false;
    $("rapidocr_modal_cancel").textContent = "取消";
    $("rapidocr_modal_start").style.display = "";
    $("rapidocr_modal_start").disabled = mode === "auto" && !automaticAvailable;
    $("rapidocr_modal_start").textContent = mode === "local" ? "选择 ZIP 并安装" : "开始安装";
  }

  function closeInstallWizard() {
    if (installInFlight) return;
    $("rapidocr_modal").classList.remove("open");
    $("rapidocr_modal").setAttribute("aria-hidden", "true");
  }

  async function startInstall() {
    if (installInFlight) return;
    installInFlight = true;
    renderRapidOcr();
    setWorkingState();
    try {
      const command = installMode === "local"
        ? "install_rapidocr_component_from_file"
        : "download_install_rapidocr_component";
      const installed = await TF.invoke(command);
      if (!installed) {
        installInFlight = false;
        renderRapidOcr();
        openInstallWizard("local");
        return;
      }
      rapidStatus = installed;
      showInstallSuccess(installed);
      fillSourceLanguages($("source_lang").value);
      showStatus("增强 OCR 已安装 ✓");
    } catch (error) {
      showInstallFailure(String(error));
      showStatus("安装失败", true);
    } finally {
      installInFlight = false;
      if (rapidStatus) rapidStatus.installing = false;
      renderRapidOcr();
    }
  }

  function setWorkingState() {
    $("rapidocr_modal_icon").textContent = "…";
    $("rapidocr_modal_icon").className = "install-hero";
    $("rapidocr_modal_title").textContent = "正在安装增强离线 OCR";
    $("rapidocr_modal_summary").textContent =
      installMode === "local" ? "已打开组件包选择窗口。选择后会立即校验并安装。" : "请保持太阳穴运行，安装完成后会明确告诉你结果。";
    $("rapidocr_progress_wrap").classList.add("visible");
    $("rapidocr_modal_error").classList.remove("visible");
    $("rapidocr_modal_cancel").disabled = true;
    $("rapidocr_modal_cancel").textContent = "安装中，请稍候";
    $("rapidocr_modal_start").style.display = "none";
    resetInstallSteps();
    setStep(installMode === "local" ? "verifying" : "downloading");
  }

  function resetInstallSteps() {
    for (const id of ["rapidocr_step_download", "rapidocr_step_verify", "rapidocr_step_install"]) {
      $(id).className = "";
    }
  }

  function setStep(stage, failed) {
    resetInstallSteps();
    const download = $("rapidocr_step_download");
    const verify = $("rapidocr_step_verify");
    const install = $("rapidocr_step_install");
    if (stage === "downloading") {
      download.className = failed ? "failed" : "active";
    } else if (stage === "verifying") {
      download.className = "done";
      verify.className = failed ? "failed" : "active";
    } else if (stage === "installing") {
      download.className = "done";
      verify.className = "done";
      install.className = failed ? "failed" : "active";
    } else if (stage === "success") {
      download.className = "done";
      verify.className = "done";
      install.className = "done";
    }
  }

  function updateInstallProgress(progress) {
    if (!progress.stage) return;
    const stage = progress.stage === "connecting" ? "downloading" : progress.stage;
    if (stage !== "failed" && stage !== "success") {
      lastInstallStage = stage;
    }
    if (!$("rapidocr_modal").classList.contains("open")) return;
    const percent = Math.max(0, Math.min(100, Number(progress.percent) || 0));
    $("rapidocr_progress_wrap").classList.add("visible");
    $("rapidocr_progress_bar").style.width = percent + "%";
    $("rapidocr_progress_percent").textContent = percent + "%";
    let detail = progress.message || "处理中…";
    if ((stage === "downloading" || stage === "installing") && progress.total_bytes) {
      detail += "  " + formatSize(progress.downloaded_bytes) + " / " + formatSize(progress.total_bytes);
    }
    $("rapidocr_progress_detail").textContent = detail;
    if (progress.source) {
      $("rapidocr_source_name").textContent = progress.source;
      $("rapidocr_source_name").title = progress.source;
    }
    if (stage === "failed") {
      setStep(lastInstallStage, true);
    } else {
      setStep(stage);
    }
  }

  function showInstallSuccess(installed) {
    updateInstallProgress({
      stage: "success",
      percent: 100,
      message: "安装完成",
    });
    $("rapidocr_modal_icon").textContent = "✓";
    $("rapidocr_modal_icon").className = "install-hero success";
    $("rapidocr_modal_title").textContent = "增强离线 OCR 安装成功";
    $("rapidocr_modal_summary").textContent =
      "版本 " + installed.version + " 已启用，支持 " + installed.supported_languages.length + " 种文字。以后截图会优先在本机识别。";
    $("rapidocr_modal_error").classList.remove("visible");
    $("rapidocr_modal_cancel").disabled = false;
    $("rapidocr_modal_cancel").textContent = "完成";
    $("rapidocr_modal_start").style.display = "none";
  }

  function showInstallFailure(error) {
    updateInstallProgress({
      stage: "failed",
      percent: 0,
      message: "安装未完成",
    });
    $("rapidocr_modal_icon").textContent = "!";
    $("rapidocr_modal_icon").className = "install-hero failed";
    $("rapidocr_modal_title").textContent = "增强离线 OCR 安装失败";
    $("rapidocr_modal_summary").textContent = "没有改动现有组件。你可以直接重试，或者关闭后使用本地 ZIP 安装。";
    $("rapidocr_modal_error").textContent = error;
    $("rapidocr_modal_error").classList.add("visible");
    $("rapidocr_modal_cancel").disabled = false;
    $("rapidocr_modal_cancel").textContent = "关闭";
    $("rapidocr_modal_start").style.display = "";
    $("rapidocr_modal_start").disabled = false;
    $("rapidocr_modal_start").textContent = "重试";
  }

  async function uninstallRapidOcr() {
    if (!confirm("卸载增强离线 OCR？主程序和设置不会被删除。")) return;
    const button = $("rapidocr_uninstall");
    button.disabled = true;
    $("rapidocr_busy").textContent = "正在卸载…";
    try {
      rapidStatus = await TF.invoke("uninstall_rapidocr_component");
      fillSourceLanguages($("source_lang").value);
      renderRapidOcr();
      showStatus("增强 OCR 已卸载");
    } catch (e) {
      showStatus("卸载失败：" + e, true);
    } finally {
      button.disabled = false;
      $("rapidocr_busy").textContent = "";
    }
  }

  function updateMmToggle() {
    const on = $("use_multimodal").checked;
    $("mm_fields").classList.toggle("disabled", !on);
  }

  function collect() {
    return {
      llm_provider: $("llm_provider").value,
      llm_base_url: $("llm_base_url").value.trim(),
      llm_api_key: $("llm_api_key").value.trim(),
      llm_model: $("llm_model").value.trim(),
      use_multimodal: $("use_multimodal").checked,
      mm_provider: $("mm_provider").value,
      mm_base_url: $("mm_base_url").value.trim(),
      mm_api_key: $("mm_api_key").value.trim(),
      mm_model: $("mm_model").value.trim(),
      native_lang: $("native_lang").value,
      target_lang: $("target_lang").value,
      source_lang: $("source_lang").value,
      use_rapidocr: $("use_rapidocr").checked,
      hotkey: $("hotkey").value.trim() || "Alt+Z",
      multimodal_threshold: parseInt($("multimodal_threshold").value) || 5,
    };
  }

  async function save() {
    const c = collect();
    try {
      await TF.invoke("save_config", { cfg: c });
      cfg = c;
      showStatus("已保存 ✓");
    } catch (e) {
      showStatus("保存失败：" + e, true);
    }
  }

  let statusTimer = null;
  function showStatus(msg, isError) {
    const s = $("status");
    s.textContent = msg;
    s.style.color = isError ? "var(--danger)" : "var(--success)";
    s.classList.add("show");
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => s.classList.remove("show"), 2500);
  }

  TF.listen("refresh-settings", () => init());
  init();
})();
