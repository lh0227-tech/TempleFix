/* ===== 首次启动欢迎向导 ===== */
(function () {
  "use strict";

  const steps = ["welcome", "service", "ocr", "finish"];
  const $ = (id) => document.getElementById(id);
  let currentStep = 0;
  let config = null;
  let providers = [];
  let rapidStatus = null;
  let connectionVerified = false;
  let languageSaveQueue = Promise.resolve();

  async function init() {
    try {
      const result = await Promise.all([
        TF.invoke("get_config"),
        TF.invoke("list_providers"),
        TF.invoke("rapidocr_status"),
      ]);
      config = result[0];
      providers = result[1];
      rapidStatus = result[2];
      fillProviders();
      fillForm();
      const initialLanguage = TFI18n.detect(config);
      $("ui_language").value = initialLanguage;
      applyLanguage(initialLanguage);
      renderOcrStatus();
      renderStep();
    } catch (error) {
      config = config || {};
      showTestStatus(String(error), true);
    }
  }

  function fillProviders() {
    const select = $("llm_provider");
    const selected = select.value;
    select.textContent = "";
    providers.forEach((provider) => {
      const option = document.createElement("option");
      option.value = provider.id;
      const localized = TFI18n.t("provider_" + provider.id);
      option.textContent = localized === "provider_" + provider.id ? provider.name : localized;
      option.dataset.baseUrl = provider.default_base_url || "";
      select.appendChild(option);
    });
    if (selected) select.value = selected;
  }

  function fillForm() {
    $("llm_provider").value = config.llm_provider || "custom";
    $("llm_base_url").value = config.llm_base_url || "https://api.deepseek.com";
    $("llm_model").value = config.llm_model || "deepseek-v4-flash";
    $("llm_api_key").value = config.llm_api_key || "";
    $("native_language").value = config.native_lang || "简体中文";
  }

  function applyLanguage(value) {
    TFI18n.setLanguage(value);
    TFI18n.apply(document);
    document.title = TFI18n.t("app_name");
    fillProviders();
    renderOcrStatus();
    renderFinishStatus();
  }

  function selectInterfaceLanguage(value) {
    applyLanguage(value);
    if (!config) return;

    config = Object.assign({}, config, { ui_language: value });
    const status = $("language_status");
    status.textContent = "";
    languageSaveQueue = languageSaveQueue
      .catch(() => {})
      .then(() => TF.invoke("save_ui_language", { uiLanguage: value }))
      .catch((error) => {
        if ($("ui_language").value === value) {
          status.textContent = TFI18n.t("save_failed") + ": " + error;
        }
      });
  }

  function renderOcrStatus() {
    if (!rapidStatus) return;
    const installed = Boolean(rapidStatus.installed);
    $("ocr_dot").classList.toggle("installed", installed);
    $("ocr_status").textContent = TFI18n.t(installed ? "ocr_installed" : "ocr_not_installed");
  }

  function hasCompleteService(values) {
    return Boolean(values.llm_base_url && values.llm_model && values.llm_api_key);
  }

  function collect(serviceFromForm) {
    const result = Object.assign({}, config || {});
    result.ui_language = $("ui_language").value;
    result.native_lang = $("native_language").value;
    if (serviceFromForm) {
      result.llm_provider = $("llm_provider").value;
      result.llm_base_url = $("llm_base_url").value.trim();
      result.llm_model = $("llm_model").value.trim();
      result.llm_api_key = $("llm_api_key").value.trim();
    }
    return result;
  }

  function renderFinishStatus() {
    if (!$("finish_service")) return;
    const ready = hasCompleteService(collect(true));
    $("finish_service").textContent = TFI18n.t(ready ? "finish_service_ready" : "finish_service_later");
  }

  function renderStep() {
    document.querySelectorAll(".step").forEach((element) => {
      element.classList.toggle("active", element.dataset.step === steps[currentStep]);
    });
    document.querySelectorAll(".progress span").forEach((element, index) => {
      element.classList.toggle("active", index <= currentStep);
    });
    $("back").classList.toggle("hidden", currentStep === 0);
    $("next").classList.toggle("hidden", currentStep === steps.length - 1);
    $("finish").classList.toggle("hidden", currentStep !== steps.length - 1);
    renderFinishStatus();
  }

  async function goTo(step) {
    const index = steps.indexOf(step);
    currentStep = index >= 0 ? index : 0;
    if (steps[currentStep] === "ocr") {
      try {
        rapidStatus = await TF.invoke("rapidocr_status");
        renderOcrStatus();
      } catch (_) {
        // 状态刷新失败不阻塞向导，保留上次结果。
      }
    }
    renderStep();
  }

  async function skip() {
    const nextConfig = collect(false);
    nextConfig.onboarding_state = "skipped";
    try {
      await TF.invoke("save_config", { cfg: nextConfig });
      config = nextConfig;
      await TF.invoke("hide_onboarding");
    } catch (error) {
      showTestStatus(TFI18n.t("save_failed") + ": " + error, true);
    }
  }

  async function finish() {
    const nextConfig = collect(true);
    nextConfig.onboarding_state = hasCompleteService(nextConfig) ? "completed" : "skipped";
    $("finish").disabled = true;
    try {
      await TF.invoke("save_config", { cfg: nextConfig });
      config = nextConfig;
      await TF.invoke("hide_onboarding");
    } catch (error) {
      showTestStatus(TFI18n.t("save_failed") + ": " + error, true);
    } finally {
      $("finish").disabled = false;
    }
  }

  async function testConnection() {
    const candidate = collect(true);
    if (!hasCompleteService(candidate)) {
      showTestStatus(TFI18n.t("required_fields"), true);
      return;
    }
    const button = $("test_connection");
    button.disabled = true;
    showTestStatus(TFI18n.t("testing"), false);
    try {
      await TF.invoke("test_llm_connection", { cfg: candidate });
      connectionVerified = true;
      showTestStatus(TFI18n.t("test_ok"), false, true);
    } catch (error) {
      connectionVerified = false;
      showTestStatus(String(error), true);
    } finally {
      button.disabled = false;
    }
  }

  function showTestStatus(message, isError, isOk) {
    const status = $("test_status");
    status.textContent = message;
    status.classList.toggle("error", Boolean(isError));
    status.classList.toggle("ok", Boolean(isOk));
  }

  function invalidateConnectionTest() {
    if (!connectionVerified && !$("test_status").textContent) return;
    connectionVerified = false;
    showTestStatus("", false);
  }

  $("ui_language").addEventListener("change", (event) => {
    selectInterfaceLanguage(event.target.value);
  });
  $("llm_provider").addEventListener("change", (event) => {
    const provider = providers.find((item) => item.id === event.target.value);
    if (provider && provider.default_base_url) $("llm_base_url").value = provider.default_base_url;
    invalidateConnectionTest();
  });
  ["llm_base_url", "llm_model", "llm_api_key"].forEach((id) => {
    $(id).addEventListener("input", invalidateConnectionTest);
  });
  $("test_connection").addEventListener("click", testConnection);
  $("open_preferences").addEventListener("click", () => TF.invoke("open_settings_window"));
  $("skip").addEventListener("click", skip);
  $("back").addEventListener("click", () => {
    currentStep = Math.max(0, currentStep - 1);
    renderStep();
  });
  $("next").addEventListener("click", () => {
    currentStep = Math.min(steps.length - 1, currentStep + 1);
    renderStep();
  });
  $("finish").addEventListener("click", finish);
  TF.listen("open-onboarding", (event) => goTo(event.payload || "welcome"));
  init();
})();
