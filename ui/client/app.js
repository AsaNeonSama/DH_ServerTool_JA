(function () {
  "use strict";

  const MAX_LOG_ENTRIES = 600;
  const SAVE_DEBOUNCE_MS = 220;
  const VIEW_PREFS_STORAGE_KEY = "dh_client_launcher_view_prefs_v1";
  const VALID_LOG_FILTERS = ["ALL", "DLL", "ERR"];

  function loadViewPrefs() {
    try {
      const raw = window.localStorage.getItem(VIEW_PREFS_STORAGE_KEY);
      if (!raw) {
        return { logFilter: "ALL", autoScroll: true };
      }
      const parsed = JSON.parse(raw);
      const filter =
        typeof parsed.logFilter === "string" && VALID_LOG_FILTERS.indexOf(parsed.logFilter) >= 0
          ? parsed.logFilter
          : "ALL";
      return {
        logFilter: filter,
        autoScroll: typeof parsed.autoScroll === "boolean" ? parsed.autoScroll : true,
      };
    } catch (_) {
      return { logFilter: "ALL", autoScroll: true };
    }
  }

  function saveViewPrefs() {
    try {
      window.localStorage.setItem(
        VIEW_PREFS_STORAGE_KEY,
        JSON.stringify({
          logFilter: state.logFilter,
          autoScroll: Boolean(els.autoScrollToggle && els.autoScrollToggle.checked),
        })
      );
    } catch (_) {}
  }

  function installReloadAndContextMenuGuards() {
    document.addEventListener(
      "contextmenu",
      function (event) {
        event.preventDefault();
      },
      true
    );

    window.addEventListener(
      "keydown",
      function (event) {
        const key = (event.key || "").toLowerCase();
        const isF5 = key === "f5" || event.keyCode === 116;
        const isF12 = key === "f12" || event.keyCode === 123;
        const isCtrlR = (event.ctrlKey || event.metaKey) && key === "r";
        const isDevToolsShortcut =
          (event.ctrlKey || event.metaKey) && event.shiftKey && (key === "i" || key === "j" || key === "c");
        if (isF5 || isF12 || isCtrlR || isDevToolsShortcut) {
          event.preventDefault();
          event.stopPropagation();
        }
      },
      true
    );
  }

  const savedPrefs = loadViewPrefs();

  const state = {
    config: {
      exePath: "./DreadHunger/Binaries/Win64/DreadHunger-Win64-Shipping.exe",
      builtin: {
        eosFixPlan: "plan_b",
        voiceChat: false,
      },
    },
    statuses: {
      exeExists: false,
    },
    licenseUrl: "https://github.com/AsaNeonSama/DH_Tool_JA/blob/main/LICENSE",
    licenseText: null,
    process: {
      status: "idle",
      pid: null,
      exitCode: null,
      modeLabel: "内蔵DLL (1件 / Frida未使用)",
    },
    logs: [],
    logFilter: savedPrefs.logFilter,
    saveTimer: null,
    isLaunching: false,
  };

  const els = {
    topNeonRibbon: document.getElementById("topNeonRibbon"),
    appHeader: document.getElementById("appHeader"),
    headerStatusBadge: document.getElementById("headerStatusBadge"),
    winMinBtn: document.getElementById("winMinBtn"),
    winCloseBtn: document.getElementById("winCloseBtn"),
    exePathInput: document.getElementById("exePathInput"),
    exeStatusPill: document.getElementById("exeStatusPill"),
    pickExeBtn: document.getElementById("pickExeBtn"),
    autoDetectExeBtn: document.getElementById("autoDetectExeBtn"),
    refreshStatusBtn: document.getElementById("refreshStatusBtn"),
    builtinCountPill: document.getElementById("builtinCountPill"),
    builtinPlanABtn: document.getElementById("builtinPlanABtn"),
    builtinPlanBBtn: document.getElementById("builtinPlanBBtn"),
    builtinVoiceBtn: document.getElementById("builtinVoiceBtn"),
    launchModeBadge: document.getElementById("launchModeBadge"),
    processDetailText: document.getElementById("processDetailText"),
    launchBtn: document.getElementById("launchBtn"),
    terminateBtn: document.getElementById("terminateBtn"),
    logCountPill: document.getElementById("logCountPill"),
    logFeed: document.getElementById("logFeed"),
    autoScrollToggle: document.getElementById("autoScrollToggle"),
    copyLogBtn: document.getElementById("copyLogBtn"),
    clearLogBtn: document.getElementById("clearLogBtn"),
    footerLicenseBtn: document.getElementById("footerLicenseBtn"),
    footerVersion: document.getElementById("footerVersion"),
    licenseModalBackdrop: document.getElementById("licenseModalBackdrop"),
    licenseModalText: document.getElementById("licenseModalText"),
    licenseCloseBtn: document.getElementById("licenseCloseBtn"),
    toastContainer: document.getElementById("toastContainer"),
  };

  function hasTauri() {
    return Boolean(window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke);
  }

  async function invokeCmd(name, args) {
    if (!hasTauri()) {
      throw new Error("Tauri バックエンドに接続できません (ブラウザ単体プレビュー中)");
    }
    return window.__TAURI__.core.invoke(name, args || {});
  }

  function showToast(message, variant) {
    if (!els.toastContainer) {
      return;
    }
    const toast = document.createElement("div");
    toast.className = "toast " + (variant || "success");
    toast.setAttribute("role", "alert");

    const msg = document.createElement("div");
    msg.className = "toast-message";
    msg.textContent = message;

    let autoTimer = null;
    function dismissToast() {
      if (!toast.isConnected || toast.dataset.closing === "1") {
        return;
      }
      toast.dataset.closing = "1";
      if (autoTimer !== null) {
        window.clearTimeout(autoTimer);
        autoTimer = null;
      }
      const reducedMotion =
        window.matchMedia && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      if (reducedMotion) {
        toast.remove();
        return;
      }
      const removeSafely = function () {
        if (toast.isConnected) {
          toast.remove();
        }
      };
      toast.addEventListener("animationend", removeSafely, { once: true });
      toast.classList.add("is-leaving");
      window.setTimeout(removeSafely, 260);
    }

    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "toast-close";
    closeBtn.setAttribute("aria-label", "閉じる");
    closeBtn.textContent = "×";
    closeBtn.addEventListener("click", dismissToast);

    toast.appendChild(msg);
    toast.appendChild(closeBtn);
    els.toastContainer.appendChild(toast);

    autoTimer = window.setTimeout(dismissToast, 3600);
  }

  function formatNow() {
    const d = new Date();
    const pad = function (n, width) {
      return String(n).padStart(width || 2, "0");
    };
    return (
      pad(d.getHours()) +
      ":" +
      pad(d.getMinutes()) +
      ":" +
      pad(d.getSeconds()) +
      "." +
      pad(d.getMilliseconds(), 3)
    );
  }

  function pushLog(entry) {
    const normalized = {
      timestamp: entry.timestamp || formatNow(),
      level: String(entry.level || "INFO").toUpperCase(),
      category: String(entry.category || "SYS").toUpperCase(),
      source: entry.source || "launcher",
      message: entry.message || "",
    };
    state.logs.push(normalized);
    if (state.logs.length > MAX_LOG_ENTRIES) {
      state.logs.splice(0, state.logs.length - MAX_LOG_ENTRIES);
    }
    renderLogs();
  }

  function ensureBuiltinConfig() {
    if (!state.config.builtin || typeof state.config.builtin !== "object") {
      state.config.builtin = { eosFixPlan: "plan_b", voiceChat: false };
    }
    return state.config.builtin;
  }

  function countEnabledBuiltins() {
    const b = ensureBuiltinConfig();
    let count = 0;
    if (b.eosFixPlan === "plan_a" || b.eosFixPlan === "plan_b") {
      count += 1;
    }
    if (b.voiceChat) {
      count += 1;
    }
    return count;
  }

  function renderExeSection() {
    if (document.activeElement !== els.exePathInput) {
      els.exePathInput.value = state.config.exePath || "";
    }
    const ok = Boolean(state.statuses.exeExists);
    els.exeStatusPill.textContent = ok ? "OK" : "未検出";
    els.exeStatusPill.className = "status-pill " + (ok ? "pill-ok" : "pill-missing");
  }

  function updateBuiltinTile(btn, id, active) {
    if (!btn) {
      return;
    }
    btn.classList.toggle("is-active", active);
    btn.setAttribute("aria-pressed", active ? "true" : "false");
    const pill = btn.querySelector('[data-state-pill="' + id + '"]');
    if (pill) {
      pill.textContent = active ? "ON" : "OFF";
    }
  }

  function renderBuiltinSection() {
    const b = ensureBuiltinConfig();
    const planAActive = b.eosFixPlan === "plan_a";
    const planBActive = b.eosFixPlan === "plan_b";
    const voiceActive = Boolean(b.voiceChat);

    updateBuiltinTile(els.builtinPlanABtn, "eos_fix_plan_a", planAActive);
    updateBuiltinTile(els.builtinPlanBBtn, "eos_fix_plan_b", planBActive);
    updateBuiltinTile(els.builtinVoiceBtn, "voice_chat", voiceActive);

    const enabledCount = countEnabledBuiltins();
    if (els.builtinCountPill) {
      els.builtinCountPill.textContent = "有効 " + enabledCount + " / 2";
    }
  }

  function computeLaunchModeLabel() {
    const builtinEnabled = countEnabledBuiltins();
    if (builtinEnabled === 0) {
      return "通常起動 (内蔵MOD無効)";
    }
    return "内蔵DLL (" + builtinEnabled + "件 / Frida未使用)";
  }

  function renderProcessStatus() {
    els.launchModeBadge.textContent = computeLaunchModeLabel();

    const p = state.process;
    const st = p.status || "idle";
    els.headerStatusBadge.className = "badge badge-status status-" + st;

    if (st === "injecting") {
      els.headerStatusBadge.textContent = "起動中...";
      els.processDetailText.textContent = "内蔵DLL展開・OEP注入中...";
      els.launchBtn.classList.add("is-injecting");
      els.launchBtn.textContent = "ゲーム起動";
      els.launchBtn.disabled = true;
      els.terminateBtn.disabled = true;
    } else if (st === "running") {
      const pidLabel = p.pid ? "PID: " + p.pid : "稼働中";
      els.headerStatusBadge.textContent = "起動中 (" + pidLabel + ")";
      els.processDetailText.textContent = "起動中 (" + pidLabel + ") — 多重起動ロック";
      els.launchBtn.classList.remove("is-injecting");
      els.launchBtn.textContent = "ゲーム起動中";
      els.launchBtn.disabled = true;
      els.terminateBtn.disabled = false;
    } else if (st === "exited") {
      const codeStr =
        p.exitCode !== null && p.exitCode !== undefined ? "CODE: " + p.exitCode : "終了";
      els.headerStatusBadge.textContent = "終了 (" + codeStr + ")";
      els.processDetailText.textContent =
        "終了 (" + (p.pid ? "PID: " + p.pid + " / " : "") + codeStr + ")";
      els.launchBtn.classList.remove("is-injecting");
      els.launchBtn.textContent = "ゲーム起動";
      els.launchBtn.disabled = Boolean(state.isLaunching);
      els.terminateBtn.disabled = true;
    } else {
      els.headerStatusBadge.textContent = "待機中";
      els.processDetailText.textContent = "待機中";
      els.launchBtn.classList.remove("is-injecting");
      els.launchBtn.textContent = "ゲーム起動";
      els.launchBtn.disabled = Boolean(state.isLaunching);
      els.terminateBtn.disabled = true;
    }
  }

  function renderLogs() {
    const filter = state.logFilter;
    const filtered = state.logs.filter(function (item) {
      if (filter === "ALL") {
        return true;
      }
      if (filter === "ERR") {
        return item.level === "ERR" || item.level === "WARN";
      }
      return item.category === filter;
    });

    els.logCountPill.textContent = filtered.length + "件";
    els.logFeed.textContent = "";

    if (filtered.length === 0) {
      const empty = document.createElement("div");
      empty.className = "log-empty";
      empty.textContent =
        state.logs.length === 0
          ? "ログ待機中..."
          : "該当するログはありません。";
      els.logFeed.appendChild(empty);
      return;
    }

    const frag = document.createDocumentFragment();
    filtered.forEach(function (item) {
      const line = document.createElement("div");
      const lvlLower = item.level.toLowerCase();
      line.className = "log-line level-" + lvlLower;

      const timeEl = document.createElement("span");
      timeEl.className = "log-time";
      timeEl.textContent = "[" + item.timestamp + "]";

      const tagEl = document.createElement("span");
      tagEl.className = "log-tag log-tag-" + lvlLower;
      tagEl.textContent = item.level;

      const catEl = document.createElement("span");
      catEl.className = "log-cat";
      catEl.textContent = "[" + item.category + "]";

      const srcEl = document.createElement("span");
      srcEl.className = "log-src";
      srcEl.textContent = item.source + ":";

      const msgEl = document.createElement("span");
      msgEl.className = "log-msg";
      msgEl.textContent = item.message;

      line.appendChild(timeEl);
      line.appendChild(tagEl);
      line.appendChild(catEl);
      line.appendChild(srcEl);
      line.appendChild(msgEl);
      frag.appendChild(line);
    });

    els.logFeed.appendChild(frag);
    if (els.autoScrollToggle && els.autoScrollToggle.checked) {
      els.logFeed.scrollTop = els.logFeed.scrollHeight;
    }
  }

  function renderAll() {
    renderExeSection();
    renderBuiltinSection();
    renderProcessStatus();
    renderLogs();
  }

  function applyStatePayload(payload, hydrateLogs) {
    if (!payload) {
      return;
    }
    if (payload.config) {
      state.config = {
        exePath: payload.config.exePath || "",
        builtin: payload.config.builtin || { eosFixPlan: "plan_b", voiceChat: false },
      };
      ensureBuiltinConfig();
    }
    if (payload.statuses) {
      state.statuses = payload.statuses;
    }
    if (payload.licenseUrl) {
      state.licenseUrl = payload.licenseUrl;
    }
    if (payload.process) {
      state.process = payload.process;
      state.isLaunching = payload.process.status === "injecting";
    }
    if (hydrateLogs && Array.isArray(payload.logs)) {
      state.logs = payload.logs.slice(-MAX_LOG_ENTRIES);
    }
    if (payload.version && els.footerVersion) {
      els.footerVersion.textContent = "DH_ClientLauncher_JA v" + payload.version;
    }
    renderAll();
  }

  function toggleBuiltinEosPlan(targetPlan) {
    const b = ensureBuiltinConfig();
    const prev = b.eosFixPlan;
    if (prev === targetPlan) {
      b.eosFixPlan = "none";
    } else {
      b.eosFixPlan = targetPlan;
      if (prev === "plan_a" && targetPlan === "plan_b") {
        showToast("EOS修復 Plan B に切り替えました (Plan A を自動OFF)。", "warn");
      } else if (prev === "plan_b" && targetPlan === "plan_a") {
        showToast("EOS修復 Plan A に切り替えました (Plan B を自動OFF)。", "warn");
      }
    }
    persistConfigNow();
  }

  function toggleBuiltinVoice() {
    const b = ensureBuiltinConfig();
    b.voiceChat = !b.voiceChat;
    persistConfigNow();
  }

  function syncDraftImmediate() {
    if (!hasTauri()) {
      return;
    }
    invokeCmd("sync_config_draft", { config: state.config })
      .then(function (statuses) {
        if (statuses) {
          state.statuses = statuses;
          renderExeSection();
        }
      })
      .catch(function () {});
  }

  function scheduleSaveConfig() {
    syncDraftImmediate();
    if (state.saveTimer !== null) {
      window.clearTimeout(state.saveTimer);
    }
    state.saveTimer = window.setTimeout(function () {
      state.saveTimer = null;
      persistConfigNow();
    }, SAVE_DEBOUNCE_MS);
  }

  async function persistConfigNow() {
    if (state.saveTimer !== null) {
      window.clearTimeout(state.saveTimer);
      state.saveTimer = null;
    }
    renderAll();
    if (!hasTauri()) {
      return;
    }
    try {
      const resp = await invokeCmd("save_config", { config: state.config });
      applyStatePayload(resp, false);
    } catch (err) {
      showToast("設定保存エラー: " + String(err), "error");
    }
  }

  async function openLicenseModal() {
    if (!els.licenseModalBackdrop || !els.licenseModalText) {
      return;
    }
    els.licenseModalBackdrop.classList.add("is-open");
    els.licenseModalBackdrop.setAttribute("aria-hidden", "false");
    if (state.licenseText !== null) {
      els.licenseModalText.textContent = state.licenseText;
      els.licenseModalText.scrollTop = 0;
      return;
    }
    els.licenseModalText.textContent = "読み込み中...";
    try {
      const text = await invokeCmd("get_license_text");
      state.licenseText = typeof text === "string" ? text : "";
      els.licenseModalText.textContent = state.licenseText || "ライセンス本文が空です。";
      els.licenseModalText.scrollTop = 0;
    } catch (err) {
      els.licenseModalText.textContent = "ライセンス読込エラー: " + String(err);
    }
  }

  function closeLicenseModal() {
    if (els.licenseModalBackdrop) {
      els.licenseModalBackdrop.classList.remove("is-open");
      els.licenseModalBackdrop.setAttribute("aria-hidden", "true");
    }
  }

  function bindHeaderDragGuard(el) {
    if (!el) {
      return;
    }
    el.addEventListener("mousedown", function (event) {
      if (event.target && event.target.closest && event.target.closest("button, input, a, .window-controls")) {
        return;
      }
      if (event.detail >= 2) {
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      if (event.button === 0 && event.detail === 1 && hasTauri()) {
        event.preventDefault();
        invokeCmd("window_start_dragging").catch(function () {});
      }
    });
    el.addEventListener("dblclick", function (event) {
      event.preventDefault();
      event.stopPropagation();
    });
  }

  function bindEvents() {
    bindHeaderDragGuard(els.appHeader);
    bindHeaderDragGuard(els.topNeonRibbon);

    if (els.footerLicenseBtn) {
      els.footerLicenseBtn.addEventListener("click", function () {
        openLicenseModal();
      });
    }

    if (els.licenseCloseBtn) {
      els.licenseCloseBtn.addEventListener("click", closeLicenseModal);
    }

    if (els.licenseModalBackdrop) {
      els.licenseModalBackdrop.addEventListener("click", function (event) {
        if (event.target === els.licenseModalBackdrop) {
          closeLicenseModal();
        }
      });
    }

    window.addEventListener("keydown", function (event) {
      if (
        event.key === "Escape" &&
        els.licenseModalBackdrop &&
        els.licenseModalBackdrop.classList.contains("is-open")
      ) {
        closeLicenseModal();
      }
    });

    if (els.winMinBtn) {
      els.winMinBtn.addEventListener("click", function () {
        if (hasTauri()) {
          invokeCmd("window_minimize").catch(function () {});
        }
      });
    }

    if (els.winCloseBtn) {
      els.winCloseBtn.addEventListener("click", function () {
        if (hasTauri()) {
          invokeCmd("window_close").catch(function () {});
        }
      });
    }

    els.exePathInput.addEventListener("input", function () {
      state.config.exePath = els.exePathInput.value;
      scheduleSaveConfig();
    });

    els.exePathInput.addEventListener("blur", function () {
      persistConfigNow();
    });

    if (els.builtinPlanABtn) {
      els.builtinPlanABtn.addEventListener("click", function () {
        toggleBuiltinEosPlan("plan_a");
      });
    }
    if (els.builtinPlanBBtn) {
      els.builtinPlanBBtn.addEventListener("click", function () {
        toggleBuiltinEosPlan("plan_b");
      });
    }
    if (els.builtinVoiceBtn) {
      els.builtinVoiceBtn.addEventListener("click", function () {
        toggleBuiltinVoice();
      });
    }

    els.pickExeBtn.addEventListener("click", async function () {
      try {
        const selected = await invokeCmd("pick_exe_file");
        if (selected) {
          state.config.exePath = selected;
          els.exePathInput.value = selected;
          await persistConfigNow();
          showToast("実行ファイルを設定しました。", "success");
        }
      } catch (err) {
        showToast("ファイル選択エラー: " + String(err), "error");
      }
    });

    els.autoDetectExeBtn.addEventListener("click", async function () {
      try {
        const res = await invokeCmd("auto_detect_exe");
        if (res && res.found) {
          state.config.exePath = res.found;
          els.exePathInput.value = res.found;
          await persistConfigNow();
          showToast("実行ファイルを自動検出しました。", "success");
        } else {
          showToast("実行ファイルが見つかりませんでした。「参照」から指定してください。", "warn");
        }
      } catch (err) {
        showToast("自動検出エラー: " + String(err), "error");
      }
    });

    els.refreshStatusBtn.addEventListener("click", async function () {
      try {
        const resp = await invokeCmd("refresh_file_statuses");
        applyStatePayload(resp, false);
        showToast("存在状態を更新しました。", "success");
      } catch (err) {
        showToast("状態確認エラー: " + String(err), "error");
      }
    });

    els.launchBtn.addEventListener("click", async function () {
      if (
        state.isLaunching ||
        state.process.status === "running" ||
        state.process.status === "injecting"
      ) {
        return;
      }
      state.isLaunching = true;
      state.process.status = "injecting";
      renderProcessStatus();
      try {
        const saved = await invokeCmd("save_config", { config: state.config });
        applyStatePayload(saved, false);
        const proc = await invokeCmd("launch_game");
        state.process = proc;
        showToast("ゲームを起動しました (PID: " + proc.pid + ")", "success");
      } catch (err) {
        showToast("起動失敗: " + String(err), "error");
        try {
          const synced = await invokeCmd("get_process_status");
          if (synced) {
            state.process = synced;
          } else {
            state.process.status = "idle";
          }
        } catch (_) {
          state.process.status = "idle";
        }
      } finally {
        state.isLaunching = false;
        renderProcessStatus();
      }
    });

    els.terminateBtn.addEventListener("click", async function () {
      try {
        const proc = await invokeCmd("terminate_game");
        state.process = proc;
        renderProcessStatus();
        showToast("ゲームプロセスを終了しました。", "warn");
      } catch (err) {
        showToast("強制終了エラー: " + String(err), "error");
      }
    });

    const filterBtns = document.querySelectorAll(".filter-chip");
    for (let i = 0; i < filterBtns.length; i++) {
      const btnFilter = filterBtns[i].getAttribute("data-filter") || "ALL";
      filterBtns[i].classList.toggle("is-active", btnFilter === state.logFilter);
      filterBtns[i].addEventListener("click", function () {
        for (let j = 0; j < filterBtns.length; j++) {
          filterBtns[j].classList.remove("is-active");
        }
        this.classList.add("is-active");
        state.logFilter = this.getAttribute("data-filter") || "ALL";
        saveViewPrefs();
        renderLogs();
      });
    }

    if (els.autoScrollToggle) {
      els.autoScrollToggle.checked = savedPrefs.autoScroll;
      els.autoScrollToggle.addEventListener("change", function () {
        saveViewPrefs();
      });
    }

    els.copyLogBtn.addEventListener("click", async function () {
      const text = state.logs
        .map(function (l) {
          return "[" + l.timestamp + "] [" + l.level + "] [" + l.category + "] " + l.source + ": " + l.message;
        })
        .join("\n");
      if (!text) {
        showToast("コピー対象のログがありません。", "warn");
        return;
      }
      try {
        if (navigator.clipboard && navigator.clipboard.writeText) {
          await navigator.clipboard.writeText(text);
          showToast("ログをコピーしました。", "success");
        } else {
          showToast("クリップボード API を利用できません。", "error");
        }
      } catch (err) {
        showToast("ログコピー失敗: " + String(err), "error");
      }
    });

    els.clearLogBtn.addEventListener("click", async function () {
      state.logs = [];
      renderLogs();
      if (hasTauri()) {
        try {
          await invokeCmd("clear_logs");
        } catch (_) {}
      }
    });
  }

  async function init() {
    installReloadAndContextMenuGuards();
    bindEvents();
    renderAll();

    if (!hasTauri()) {
      pushLog({
        level: "WARN",
        category: "SYS",
        source: "ui",
        message: "プレビューモードで表示しています (Tauri IPC 未接続)。",
      });
      return;
    }

    try {
      await window.__TAURI__.event.listen("launcher://log", function (event) {
        if (event && event.payload) {
          pushLog(event.payload);
        }
      });

      await window.__TAURI__.event.listen("launcher://status", function (event) {
        if (event && event.payload) {
          state.process = event.payload;
          state.isLaunching = event.payload.status === "injecting";
          renderProcessStatus();
        }
      });

      const initial = await invokeCmd("get_initial_state");
      applyStatePayload(initial, true);
    } catch (err) {
      showToast("初期化エラー: " + String(err), "error");
      pushLog({
        level: "ERR",
        category: "SYS",
        source: "init",
        message: String(err),
      });
    }
  }

  init();
})();
