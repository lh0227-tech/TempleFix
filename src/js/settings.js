/* ===== 设置页逻辑 ===== */
(function () {
  "use strict";

  let providers = [];
  let cfg = null;
  let rapidStatus = null;
  let rapidRelease = null;
  let appUpdateStatus = null;
  let appUpdateBusy = false;
  let appUpdateProgress = null;
  let initialized = false;
  let installMode = "auto";
  let installInFlight = false;
  let lastInstallStage = "downloading";

  const $ = (id) => document.getElementById(id);
  const tr = (key, values) => TFI18n.t(key, values);

  async function init() {
    const [config, provs, componentStatus, releaseInfo, updateStatus] = await Promise.all([
      TF.invoke("get_config"),
      TF.invoke("list_providers"),
      TF.invoke("rapidocr_status"),
      TF.invoke("rapidocr_release_info"),
      TF.invoke("get_app_update_status"),
    ]);
    cfg = config;
    providers = provs;
    rapidStatus = componentStatus;
    rapidRelease = releaseInfo;
    appUpdateStatus = updateStatus;

    TFI18n.setLanguage(TFI18n.detect(config));
    TFI18n.apply(document);
    document.title = tr("preferences_title");

    fillProviders("llm_provider");
    fillProviders("mm_provider");
    fillSourceLanguages(config.source_lang || "");
    fillForm(config);
    renderRapidOcr();
    renderAppUpdate();

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
    $("ui_language").addEventListener("change", () => applyLanguage($("ui_language").value));
    $("rapidocr_install").addEventListener("click", () => openInstallWizard("auto"));
    $("rapidocr_local_install").addEventListener("click", () => openInstallWizard("local"));
    $("rapidocr_uninstall").addEventListener("click", uninstallRapidOcr);
    $("check_app_update").addEventListener("click", checkForAppUpdate);
    $("install_app_update").addEventListener("click", installAppUpdate);
    $("rapidocr_modal_start").addEventListener("click", startInstall);
    $("rapidocr_modal_cancel").addEventListener("click", closeInstallWizard);
    $("rapidocr_modal").addEventListener("click", (event) => {
      if (event.target === $("rapidocr_modal") && !installInFlight) closeInstallWizard();
    });
    document.addEventListener("keydown", (event) => {
      if (event.key === "Escape" && !installInFlight) closeInstallWizard();
    });
    $("save").addEventListener("click", save);
    $("open_welcome").addEventListener("click", () => {
      TF.invoke("open_onboarding", { step: "welcome" });
    });

    TF.listen("rapidocr-install-progress", (event) => updateInstallProgress(event.payload || {}));
    TF.listen("app-update-status", (event) => {
      appUpdateStatus = event.payload || appUpdateStatus;
      if (cfg && appUpdateStatus) cfg.last_update_check_at = appUpdateStatus.lastCheckedAt || 0;
      renderAppUpdate();
    });
    TF.listen("app-update-progress", (event) => {
      appUpdateProgress = event.payload || {};
      renderAppUpdateProgress();
    });
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

    $("ui_language").value = TFI18n.normalize(cfg.ui_language) || TFI18n.detect(cfg);
    $("display_mode").value = cfg.display_mode || "plain_text";
    $("native_lang").value = cfg.native_lang || "简体中文";
    $("target_lang").value = cfg.target_lang || "";
    $("source_lang").value = cfg.source_lang || "";
    $("use_rapidocr").checked = cfg.use_rapidocr !== false;
    $("hotkey").value = cfg.hotkey || "Alt+Z";
    $("auto_check_updates").checked = cfg.auto_check_updates !== false;

    updateMmToggle();
  }

  function fillProviders(selectId) {
    const sel = $(selectId);
    const selected = sel.value;
    sel.innerHTML = "";
    for (const p of providers) {
      const opt = document.createElement("option");
      opt.value = p.id;
      const localized = tr("provider_" + p.id);
      opt.textContent = localized === "provider_" + p.id ? p.name : localized;
      sel.appendChild(opt);
    }
    if (selected) sel.value = selected;
  }

  function applyLanguage(value) {
    TFI18n.setLanguage(value);
    TFI18n.apply(document);
    document.title = tr("preferences_title");
    fillProviders("llm_provider");
    fillProviders("mm_provider");
    fillSourceLanguages($("source_lang").value);
    renderRapidOcr();
    renderAppUpdate();
  }

  function fillSourceLanguages(selected) {
    const displayAliases = {
      "Chinese (Simplified)": "简体中文",
      "Chinese (Traditional)": "繁體中文",
      Japanese: "日本語",
      French: "Français",
      German: "Deutsch",
      Portuguese: "Português",
      Spanish: "Español",
    };
    const common = [
      "简体中文", "繁體中文", "English", "日本語", "한국어",
      "Français", "Deutsch", "Português", "Español",
    ];
    const enhanced = rapidStatus && rapidStatus.installed
      ? rapidStatus.supported_languages || []
      : [];
    const languages = [
      ...new Set([...common, ...enhanced.map((name) => displayAliases[name] || name)]),
    ];
    const select = $("source_lang");
    select.innerHTML = "";
    const automatic = document.createElement("option");
    automatic.value = "";
    automatic.textContent = tr("auto_detect");
    select.appendChild(automatic);
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
    $("rapidocr_install").textContent = tr(status.installed ? "reinstall" : "install");
    $("rapidocr_install").disabled = busy;
    $("rapidocr_local_install").disabled = busy;
    $("rapidocr_uninstall").disabled = busy;
    $("rapidocr_busy").textContent = busy ? tr("installing") : "";

    if (status.installed) {
      const size = formatSize(status.size_bytes);
      $("rapidocr_status").textContent = tr("installed_status", {
        version: status.version,
        size: size !== "—" ? " · " + size : "",
      });
      $("source_lang_desc").textContent = tr("source_enhanced", {
        count: (status.supported_languages || []).length,
      });
    } else if (status.error) {
      $("rapidocr_status").textContent = tr("component_error", { error: status.error });
      $("source_lang_desc").textContent = tr("source_component_error");
    } else {
      $("rapidocr_status").textContent = tr("not_installed");
      $("source_lang_desc").textContent = tr("source_not_installed");
    }
  }

  function openInstallWizard(mode) {
    if (installInFlight || (rapidStatus && rapidStatus.installing)) return;
    installMode = mode;
    lastInstallStage = mode === "auto" ? "downloading" : "verifying";
    const release = rapidRelease || {};
    const sourceNames = release.source_names || [];
    const source = mode === "local" ? tr("local_zip_source") : sourceNames.join(" / ");
    const isUpdate = rapidStatus && rapidStatus.installed;
    const automaticAvailable = !!release.available;

    $("rapidocr_modal").classList.add("open");
    $("rapidocr_modal").setAttribute("aria-hidden", "false");
    $("rapidocr_modal_icon").textContent = "↓";
    $("rapidocr_modal_icon").className = "install-hero";
    $("rapidocr_modal_title").textContent = tr(isUpdate ? "update_enhanced" : "install_enhanced");
    $("rapidocr_modal_summary").textContent = mode === "local"
      ? tr("wizard_local_summary")
      : automaticAvailable
        ? tr("wizard_auto_summary")
        : tr("wizard_unavailable_summary");
    $("rapidocr_download_size").textContent = formatSize(release.package_bytes);
    $("rapidocr_installed_size").textContent = formatSize(release.installed_bytes);
    $("rapidocr_source_name").textContent = source || tr("not_released");
    $("rapidocr_source_name").title = source || tr("not_released");
    $("rapidocr_progress_wrap").classList.remove("visible");
    $("rapidocr_progress_bar").style.width = "0%";
    $("rapidocr_progress_percent").textContent = "0%";
    $("rapidocr_progress_detail").textContent = tr("preparing");
    $("rapidocr_modal_error").classList.toggle("visible", mode === "auto" && !automaticAvailable);
    $("rapidocr_modal_error").textContent = mode === "auto" && !automaticAvailable
      ? (release.error || tr("wizard_unavailable_summary"))
      : "";
    resetInstallSteps();

    $("rapidocr_modal_cancel").disabled = false;
    $("rapidocr_modal_cancel").textContent = tr("cancel");
    $("rapidocr_modal_start").style.display = "";
    $("rapidocr_modal_start").disabled = mode === "auto" && !automaticAvailable;
    $("rapidocr_modal_start").textContent = tr(mode === "local" ? "choose_zip_install" : "start_install");
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
      showStatus(tr("install_success_status"));
    } catch (error) {
      showInstallFailure(String(error));
      showStatus(tr("install_failed_status"), true);
    } finally {
      installInFlight = false;
      if (rapidStatus) rapidStatus.installing = false;
      renderRapidOcr();
    }
  }

  function setWorkingState() {
    $("rapidocr_modal_icon").textContent = "…";
    $("rapidocr_modal_icon").className = "install-hero";
    $("rapidocr_modal_title").textContent = tr("working_title");
    $("rapidocr_modal_summary").textContent =
      tr(installMode === "local" ? "working_local" : "working_auto");
    $("rapidocr_progress_wrap").classList.add("visible");
    $("rapidocr_modal_error").classList.remove("visible");
    $("rapidocr_modal_cancel").disabled = true;
    $("rapidocr_modal_cancel").textContent = tr("install_wait");
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
    const stageKey = "stage_" + stage;
    let detail = tr(stageKey);
    if (detail === stageKey) detail = progress.message || tr("processing");
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
      message: tr("stage_success"),
    });
    $("rapidocr_modal_icon").textContent = "✓";
    $("rapidocr_modal_icon").className = "install-hero success";
    $("rapidocr_modal_title").textContent = tr("success_title");
    $("rapidocr_modal_summary").textContent = tr("success_summary", {
      version: installed.version,
      count: (installed.supported_languages || []).length,
    });
    $("rapidocr_modal_error").classList.remove("visible");
    $("rapidocr_modal_cancel").disabled = false;
    $("rapidocr_modal_cancel").textContent = tr("complete");
    $("rapidocr_modal_start").style.display = "none";
  }

  function showInstallFailure(error) {
    updateInstallProgress({
      stage: "failed",
      percent: 0,
      message: tr("stage_failed"),
    });
    $("rapidocr_modal_icon").textContent = "!";
    $("rapidocr_modal_icon").className = "install-hero failed";
    $("rapidocr_modal_title").textContent = tr("failure_title");
    $("rapidocr_modal_summary").textContent = tr("failure_summary");
    $("rapidocr_modal_error").textContent = error;
    $("rapidocr_modal_error").classList.add("visible");
    $("rapidocr_modal_cancel").disabled = false;
    $("rapidocr_modal_cancel").textContent = tr("close");
    $("rapidocr_modal_start").style.display = "";
    $("rapidocr_modal_start").disabled = false;
    $("rapidocr_modal_start").textContent = tr("retry");
  }

  async function uninstallRapidOcr() {
    if (!confirm(tr("uninstall_confirm"))) return;
    const button = $("rapidocr_uninstall");
    button.disabled = true;
    $("rapidocr_busy").textContent = tr("uninstalling");
    try {
      rapidStatus = await TF.invoke("uninstall_rapidocr_component");
      fillSourceLanguages($("source_lang").value);
      renderRapidOcr();
      showStatus(tr("uninstalled"));
    } catch (e) {
      showStatus(tr("uninstall_failed", { error: e }), true);
    } finally {
      button.disabled = false;
      $("rapidocr_busy").textContent = "";
    }
  }

  function updateStatusText(status) {
    if (!status) return tr("update_not_checked");
    if (!status.configured) return tr("update_not_configured");
    if (status.checking || appUpdateBusy === "checking") return tr("checking_updates");
    if (appUpdateBusy === "installing") return tr("installing_app_update");
    if (status.error) return tr("update_check_failed");
    if (status.available) {
      return tr("update_available", { version: status.version || "" });
    }
    if (status.lastCheckedAt) return tr("up_to_date");
    return tr("update_not_checked");
  }

  function formatUpdateTime(timestamp) {
    if (!timestamp) return tr("update_never");
    try {
      return new Date(timestamp * 1000).toLocaleString(document.documentElement.lang || undefined);
    } catch (_) {
      return tr("update_never");
    }
  }

  function renderAppUpdate() {
    if (!$("app_update_card")) return;
    const status = appUpdateStatus || {};
    const dot = $("app_update_dot");
    const busy = Boolean(appUpdateBusy) || Boolean(status.checking);

    $("app_version").textContent = status.currentVersion || "—";
    $("app_update_status").textContent = updateStatusText(status);
    dot.className = "status-dot";
    if (busy) dot.classList.add("busy");
    else if (status.error && status.configured) dot.classList.add("broken");
    else if (status.available) dot.classList.add("available");
    else if (status.lastCheckedAt) dot.classList.add("ready");

    const checkButton = $("check_app_update");
    const installButton = $("install_app_update");
    checkButton.disabled = busy || !status.configured;
    checkButton.textContent = tr(status.checking || appUpdateBusy === "checking"
      ? "checking_updates"
      : "check_for_updates");
    installButton.style.display = status.available ? "" : "none";
    installButton.disabled = busy;

    $("app_update_source_wrap").style.display = status.source ? "" : "none";
    $("app_update_source").textContent = status.source || "—";
    $("app_update_checked_wrap").style.display = status.lastCheckedAt ? "" : "none";
    $("app_update_checked").textContent = formatUpdateTime(status.lastCheckedAt);

    const error = $("app_update_error");
    const showError = Boolean(status.error && status.configured);
    error.textContent = showError ? String(status.error) : "";
    error.classList.toggle("visible", showError);

    const notes = $("app_update_notes_wrap");
    const hasNotes = Boolean(status.available && status.notes);
    $("app_update_notes").textContent = hasNotes ? status.notes : "";
    notes.classList.toggle("visible", hasNotes);

    if (!appUpdateBusy && !appUpdateProgress) {
      $("app_update_progress").classList.remove("visible");
    }
  }

  function renderAppUpdateProgress() {
    if (!appUpdateProgress) return;
    const stage = appUpdateProgress.stage || "downloading";
    const percent = Math.max(0, Math.min(100, Number(appUpdateProgress.percent) || 0));
    const key = stage === "verifying"
      ? "verifying_app_update"
      : stage === "installing"
        ? "installing_app_update"
        : "downloading_app_update";
    $("app_update_progress").classList.add("visible");
    $("app_update_progress_bar").style.width = percent + "%";
    $("app_update_progress_percent").textContent = stage === "downloading" ? percent + "%" : "";
    $("app_update_progress_text").textContent = tr(key);
  }

  async function checkForAppUpdate() {
    if (appUpdateBusy) return;
    appUpdateBusy = "checking";
    appUpdateProgress = null;
    renderAppUpdate();
    try {
      appUpdateStatus = await TF.invoke("check_app_update");
      if (cfg) cfg.last_update_check_at = appUpdateStatus.lastCheckedAt || 0;
    } catch (error) {
      appUpdateStatus = Object.assign({}, appUpdateStatus, {
        checking: false,
        error: String(error),
      });
    } finally {
      appUpdateBusy = false;
      renderAppUpdate();
    }
  }

  async function installAppUpdate() {
    if (appUpdateBusy || !appUpdateStatus || !appUpdateStatus.available) return;
    if (!window.confirm(tr("update_install_confirm", { version: appUpdateStatus.version || "" }))) {
      return;
    }
    appUpdateBusy = "installing";
    appUpdateProgress = { stage: "downloading", percent: 0 };
    renderAppUpdate();
    renderAppUpdateProgress();
    try {
      await TF.invoke("install_app_update");
      $("app_update_progress_text").textContent = tr("update_restart_pending");
    } catch (error) {
      appUpdateStatus = Object.assign({}, appUpdateStatus, { error: String(error) });
      appUpdateProgress = null;
      appUpdateBusy = false;
      renderAppUpdate();
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
      display_mode: $("display_mode").value,
      ui_language: $("ui_language").value,
      onboarding_state: cfg.onboarding_state ?? null,
      auto_check_updates: $("auto_check_updates").checked,
      last_update_check_at: cfg.last_update_check_at || 0,
    };
  }

  async function save() {
    const c = collect();
    try {
      await TF.invoke("save_config", { cfg: c });
      cfg = c;
      showStatus(tr("saved"));
    } catch (e) {
      showStatus(tr("save_error", { error: e }), true);
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
