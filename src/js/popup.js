/* ===== 翻译结果浮窗 ===== */
(function () {
  "use strict";

  const $ = (id) => document.getElementById(id);
  const tr = (key, values) => TFI18n.t(key, values);
  let curResult = null;
  let targetLang = "";

  // 1. 最先绑定所有交互（不依赖异步数据，保证按钮/ESC 永远可用）
  function bindActions() {
    $("close").addEventListener("click", close);
    $("copy").addEventListener("click", copyText);
    $("copy2").addEventListener("click", copyText);
    $("retranslate").addEventListener("click", retry);

    // 右键也能关（双保险）
    document.addEventListener("contextmenu", (e) => {
      e.preventDefault();
      close();
    });

    // 前端 ESC（窗口有焦点时生效；全局 ESC 由后端热键兜底）
    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") close();
    });

    // 头部拖动
    const header = document.querySelector(".header");
    header.addEventListener("mousedown", (e) => {
      if (e.target.closest(".icon-btn")) return;
      const w = window.__TAURI__?.window?.getCurrentWindow?.();
      if (w) w.startDragging();
    });
  }

  // 2. 再异步加载数据
  async function loadData() {
    try {
      const [data, config] = await Promise.all([
        TF.invoke("get_last_result"),
        TF.invoke("get_config"),
      ]);
      TFI18n.setLanguage(TFI18n.detect(config));
      TFI18n.apply(document);
      document.title = tr("popup_title");
      targetLang = (data && data.target_lang) || "简体中文";
      render(data && data.result);
    } catch (e) {
      showLoading(tr("translate_failed") + ": " + e);
    }
  }

  // 监听刷新事件（每次 show 时后端 emit，触发重新加载最新结果）
  TF.listen("refresh-result", () => {
    loadData();
  });

  function render(result) {
    if (!result) {
      showLoading(tr("translating"));
      return;
    }
    curResult = result;
    const content = $("content");

    if (!result.success) {
      if (["NO_KEY", "NO_MODEL", "NO_BASE_URL"].includes(result.error_code)) {
        renderMissingService(result);
        return;
      }
      content.innerHTML = `
        <div class="error-box">
          <div class="msg">${esc(localizedError(result))}</div>
          <button class="btn btn-primary retry" id="retryBtn">${esc(tr("retry"))}</button>
        </div>`;
      const rb = $("retryBtn");
      if (rb) rb.addEventListener("click", retry);
      fitWindow(result);
      return;
    }

    // 短OCR提示：原文太少，建议用多模态
    const isShortOcr = result.error_code === "SHORT_OCR";

    content.innerHTML = `
      <div class="block">
        <div class="label">${esc(tr("original"))} ${isShortOcr ? esc(tr("recognition_short")) : ""}</div>
        <div class="text ${result.original ? "" : "empty"}">${
      esc(result.original) || esc(tr("no_text"))
    }</div>
      </div>
      <div class="block">
        <div class="label">${esc(tr("translation_to", { language: targetLang }))}</div>
        <div class="text ${result.translated ? "" : "empty"}">${
      esc(result.translated) || esc(tr("no_text"))
    }</div>
      </div>`;

    // 短OCR：加"用大模型重试"按钮
    if (isShortOcr) {
      const btn = document.createElement("button");
      btn.className = "btn btn-primary mm-retry-btn";
      btn.textContent = tr("mm_retry");
      btn.addEventListener("click", mmRetry);
      content.appendChild(btn);
    }

    const t = $("title");
    if (t) t.textContent = result.model || "太阳穴";
    $("copy2").textContent = tr("copy_translation");
    fitWindow(result);
  }

  function renderMissingService(result) {
    curResult = result;
    const original = result.original || "";
    $("content").innerHTML = `
      <div class="setup-required">
        <div class="setup-title">${esc(tr("missing_title"))}</div>
        <div class="setup-copy">${esc(tr("missing_copy"))}</div>
        ${original ? `<div class="ocr-preview"><div class="label">${esc(tr("recognized_text"))}</div><div class="text">${esc(original)}</div></div>` : ""}
        <div class="setup-actions">
          <button class="btn btn-primary" id="setupNow">${esc(tr("setup_now"))}</button>
          <button class="btn btn-ghost" id="ocrOnly" ${original ? "" : "disabled"}>${esc(tr("extract_only"))}</button>
          <button class="btn btn-ghost" id="setupCancel">${esc(tr("cancel"))}</button>
        </div>
      </div>`;
    $("setupNow").addEventListener("click", openSetup);
    $("ocrOnly").addEventListener("click", () => renderOcrOnly(original));
    $("setupCancel").addEventListener("click", close);
    fitWindow(result);
  }

  function renderOcrOnly(original) {
    curResult = {
      success: true,
      original,
      translated: "",
      model: "OCR",
    };
    $("content").innerHTML = `
      <div class="block">
        <div class="label">${esc(tr("extracted_text"))}</div>
        <div class="text">${esc(original)}</div>
      </div>`;
    const title = $("title");
    if (title) title.textContent = tr("extract_title");
    const copy = $("copy2");
    if (copy) copy.textContent = tr("copy_text");
    fitWindow(curResult);
  }

  async function openSetup() {
    try {
      await TF.invoke("open_onboarding", { step: "service" });
      await TF.invoke("hide_popup");
    } catch (e) {
      showLoading(tr("open_settings_failed", { error: e }));
    }
  }

  function measureLongestLine(result) {
    const sample = document.querySelector(".block .text");
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (sample && ctx) ctx.font = getComputedStyle(sample).font;
    const values = [result && result.original, result && result.translated].filter(Boolean);
    let longest = 0;
    for (const value of values) {
      for (const line of String(value).split(/\r?\n/)) {
        longest = Math.max(longest, ctx ? ctx.measureText(line).width : line.length * 15);
      }
    }
    return longest;
  }

  async function fitWindow(result) {
    // 先让新内容完成排版，再用不可见副本测自然高度。
    await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    const width = Math.max(360, Math.min(720, Math.ceil(measureLongestLine(result) + 72)));
    const popup = document.querySelector(".popup");
    if (!popup) return;
    const probe = popup.cloneNode(true);
    probe.classList.add("size-probe");
    probe.style.width = width + "px";
    document.body.appendChild(probe);
    const height = Math.max(180, Math.min(760, Math.ceil(probe.scrollHeight + 2)));
    probe.remove();
    try {
      await TF.invoke("resize_popup", { width, height });
    } catch (_) {
      // 调整失败不影响翻译内容本身，继续保留当前窗口大小。
    }
  }

  // 用多模态重新翻译（用户点"用大模型重新识别"）
  async function mmRetry() {
    showLoading(tr("mm_working"));
    try {
      const r = await TF.invoke("multimodal_retry");
      render(r);
    } catch (e) {
      showLoading(tr("recognition_failed", { error: e }));
    }
  }

  function showLoading(msg) {
    $("content").innerHTML = `
      <div class="loading">
        <div class="spinner"></div>
        <span>${esc(msg)}</span>
      </div>`;
  }

  function copyText() {
    const text = curResult && (curResult.translated || curResult.original);
    if (!text) return;
    navigator.clipboard.writeText(text).then(() => {
      flashTitle(tr("copied"));
    });
  }

  async function retry() {
    try {
      await TF.invoke("trigger_screenshot_cmd");
    } catch (e) {
      showLoading(tr("recapture_failed", { error: e }));
    }
  }

  function close() {
    // 隐藏浮窗（不销毁，复用），由后端处理
    TF.invoke("hide_popup");
  }

  let titleTimer = null;
  function flashTitle(msg) {
    const t = $("title");
    if (!t) return;
    const old = t.textContent;
    t.textContent = msg;
    clearTimeout(titleTimer);
    titleTimer = setTimeout(() => (t.textContent = old), 1500);
  }

  function esc(s) {
    const d = document.createElement("div");
    d.textContent = s || "";
    return d.innerHTML;
  }

  function localizedError(result) {
    const keys = {
      AUTH: "error_auth",
      QUOTA: "error_quota",
      RATE: "error_rate",
      TIMEOUT: "error_timeout",
      NET: "error_network",
      HTTP: "error_http",
      OCR_EMPTY: "error_ocr_empty",
      OCR_UNAVAILABLE: "error_ocr_unavailable",
      PARSE: "error_parse",
      NO_MM_KEY: "error_multimodal_missing",
      NO_MM_MODEL: "error_multimodal_missing",
    };
    const key = keys[result && result.error_code];
    return key ? tr(key) : (result && result.error) || tr("translate_failed");
  }

  // 启动：先绑定，再加载
  bindActions();
  loadData();
})();
