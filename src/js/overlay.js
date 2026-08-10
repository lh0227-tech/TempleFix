/* ===== 截图遮罩选区交互 ===== */
(function () {
  "use strict";

  // ===== 状态 =====
  let shotW = 0,
    shotH = 0; // 截图物理尺寸（后端返回）
  let winW = 0,
    winH = 0; // 窗口逻辑尺寸
  let dragging = false;
  let startX = 0,
    startY = 0;
  let curRect = null; // {x,y,w,h} 当前选区 CSS 坐标

  // ===== DOM =====
  const mask = document.createElement("div");
  mask.className = "mask";
  const selBox = document.createElement("div");
  selBox.className = "selection";
  selBox.style.display = "none";
  const sizeTip = document.createElement("div");
  sizeTip.className = "size-tip";
  sizeTip.style.display = "none";
  const toolbar = document.createElement("div");
  toolbar.className = "toolbar glass";
  toolbar.style.display = "none";
  const mag = document.createElement("div");
  mag.className = "magnifier";
  const magCanvas = document.createElement("canvas");
  magCanvas.width = 140;
  magCanvas.height = 140;
  mag.appendChild(magCanvas);
  const crossV = document.createElement("div");
  crossV.className = "crosshair-v";
  crossV.style.display = "none";
  const crossH = document.createElement("div");
  crossH.className = "crosshair-h";
  crossH.style.display = "none";

  // 起始提示
  const hint = document.createElement("div");
  hint.className = "start-hint glass";
  hint.textContent = "按住鼠标拖拽框选翻译区域 · ESC 取消";

  function mount() {
    document.body.append(mask, crossV, crossH, selBox, sizeTip, mag, toolbar, hint);
  }

  // ===== 初始化 / 每次重新唤起时刷新截图 =====
  async function loadShot() {
    winW = window.innerWidth;
    winH = window.innerHeight;
    dragging = false;
    curRect = null;
    selBox.style.display = "none";
    sizeTip.style.display = "none";
    toolbar.style.display = "none";
    mag.style.display = "none";
    crossV.style.display = "none";
    crossH.style.display = "none";
    hint.style.display = "block";
    hint.textContent = "按住鼠标拖拽框选翻译区域 · ESC 取消";
    try {
      const size = await TF.invoke("get_shot_size");
      shotW = size[0];
      shotH = size[1];
      // 加载背景图给放大镜用
      const uri = await TF.invoke("get_full_image");
      shotBg = new Image();
      shotBg.src = uri;
    } catch (e) {
      hint.textContent = "截图失败：" + e;
    }
  }

  async function init() {
    mount();
    bindEvents();
    TF.listen("refresh-shot", loadShot);
    await loadShot();
  }

  // ===== 事件绑定 =====
  function bindEvents() {
    document.addEventListener("mousedown", onDown);
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.addEventListener("contextmenu", onCancel);
    document.addEventListener("keydown", onKey);
  }

  function isOnToolbar(target) {
    return target && (target === toolbar || toolbar.contains(target));
  }

  function onDown(e) {
    if (e.button !== 0) return; // 仅左键
    if (isOnToolbar(e.target)) return; // 点工具条不重置选区
    dragging = true;
    hint.style.display = "none";
    startX = e.clientX;
    startY = e.clientY;
    curRect = { x: startX, y: startY, w: 0, h: 0 };
    selBox.style.display = "block";
    selBox.style.left = startX + "px";
    selBox.style.top = startY + "px";
    selBox.style.width = "0px";
    selBox.style.height = "0px";
    sizeTip.style.display = "block";
    crossV.style.display = "block";
    crossH.style.display = "block";
    toolbar.style.display = "none";
    updateCross(e.clientX, e.clientY);
    showMag(e.clientX, e.clientY);
  }

  function onMove(e) {
    if (!dragging) {
      // 未拖拽时跟随鼠标显示放大镜（拖拽中也由下方逻辑绘制）
      drawMag(e.clientX, e.clientY);
      return;
    }
    let x = e.clientX;
    let y = e.clientY;
    // 限制在窗口内
    x = Math.max(0, Math.min(winW, x));
    y = Math.max(0, Math.min(winH, y));

    const rx = Math.min(x, startX);
    const ry = Math.min(y, startY);
    const rw = Math.abs(x - startX);
    const rh = Math.abs(y - startY);
    curRect = { x: rx, y: ry, w: rw, h: rh };

    selBox.style.left = rx + "px";
    selBox.style.top = ry + "px";
    selBox.style.width = rw + "px";
    selBox.style.height = rh + "px";

    // 尺寸提示（显示物理像素，更直观）
    sizeTip.textContent = `${Math.round(rw * (shotW / winW))} × ${Math.round(rh * (shotH / winH))}`;
    placeSizeTip(rx, ry, rw, rh);

    updateCross(x, y);
    drawMag(x, y);
  }

  function onUp(e) {
    if (!dragging) return;
    dragging = false;
    crossV.style.display = "none";
    crossH.style.display = "none";
    mag.style.display = "none";

    if (!curRect || curRect.w < 5 || curRect.h < 5) {
      // 太小，当作没选
      selBox.style.display = "none";
      sizeTip.style.display = "none";
      hint.style.display = "block";
      curRect = null;
      return;
    }
    showToolbar(curRect);
  }

  // 双击选区直接翻译
  document.addEventListener("dblclick", (e) => {
    if (isOnToolbar(e.target)) return;
    if (curRect && curRect.w >= 5 && curRect.h >= 5) {
      doTranslate();
    }
  });

  function placeSizeTip(rx, ry, rw, rh) {
    // 默认放选区下方
    let left = rx;
    let top = ry + rh + 6;
    if (top + 24 > winH) top = ry - 26; // 放不下就放上方
    sizeTip.style.left = left + "px";
    sizeTip.style.top = top + "px";
  }

  function updateCross(x, y) {
    crossV.style.left = x + "px";
    crossH.style.top = y + "px";
  }

  function showToolbar(rect) {
    toolbar.innerHTML = "";
    const btnT = document.createElement("button");
    btnT.className = "btn btn-primary";
    btnT.textContent = "翻译";
    btnT.onclick = doTranslate;
    const btnC = document.createElement("button");
    btnC.className = "btn btn-ghost";
    btnC.textContent = "取消";
    btnC.onclick = onCancel;
    toolbar.append(btnT, btnC);
    toolbar.style.display = "flex";

    // 默认放选区下方，放不下放上方
    let left = rect.x;
    let top = rect.y + rect.h + 8;
    const tw = 150,
      th = 50;
    if (top + th > winH) top = rect.y - th - 8;
    if (left + tw > winW) left = winW - tw - 4;
    if (left < 4) left = 4;
    toolbar.style.left = left + "px";
    toolbar.style.top = top + "px";
  }

  // ===== 放大镜 =====
  function showMag(x, y) {
    mag.style.display = "block";
    drawMag(x, y);
  }

  // 3 倍放大，跟随鼠标，显示鼠标周围像素
  function drawMag(x, y) {
    // 放大镜放在鼠标右下方，避免遮挡；到边了换位
    let mx = x + 20,
      my = y + 20;
    if (mx + 160 > winW) mx = x - 160;
    if (my + 160 > winH) my = y - 160;
    mag.style.left = mx + "px";
    mag.style.top = my + "px";

    const ctx = magCanvas.getContext("2d");
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, 140, 140);
    // 用截图当背景图源（取物理像素）
    if (shotBg && shotBg.complete) {
      const sx = (x / winW) * shotW - 140 / 6; // 3倍：140/3≈46.7，取中心
      const sy = (y / winH) * shotH - 140 / 6;
      ctx.drawImage(shotBg, sx, sy, 140 / 3, 140 / 3, 0, 0, 140, 140);
    }
    // 中心十字准星
    ctx.strokeStyle = "rgba(255,90,90,0.9)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(70, 0);
    ctx.lineTo(70, 140);
    ctx.moveTo(0, 70);
    ctx.lineTo(140, 70);
    ctx.stroke();
  }

  let shotBg = null; // 截图背景图（放大镜用）

  // ===== 翻译 =====
  async function doTranslate() {
    if (!curRect) return;
    const { x, y, w, h } = curRect;
    toolbar.style.display = "none";
    selBox.style.display = "none";
    sizeTip.style.display = "none";

    hint.style.display = "block";
    hint.textContent = "正在翻译…";

    try {
      const dataUri = await TF.invoke("crop_region", {
        cssX: x,
        cssY: y,
        cssW: w,
        cssH: h,
        winW,
        winH,
        shotW,
        shotH,
      });
      const cfg = await TF.invoke("get_config");
      const result = await TF.invoke("translate", {
        dataUri,
        targetLang: cfg.target_lang || "",
      });
      // 先开浮窗，等浮窗加载一会，再关遮罩
      // （遮罩关闭会销毁本窗口JS上下文，必须最后关；且要给浮窗加载时间）
      await TF.invoke("open_popup");
      await new Promise((r) => setTimeout(r, 300));
      await TF.invoke("close_overlay");
    } catch (e) {
      hint.style.display = "block";
      hint.textContent = "❌ " + e;
    }
  }

  function onCancel() {
    TF.invoke("close_overlay");
  }

  function onKey(e) {
    if (e.key === "Escape") onCancel();
  }

  // ===== 启动 =====
  init();
})();
