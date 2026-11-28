"use strict";

(function () {
  const tauriGlobal = window.__TAURI__;
  const invoke = tauriGlobal && tauriGlobal.core ? tauriGlobal.core.invoke : null;
  const listen = tauriGlobal && tauriGlobal.event ? tauriGlobal.event.listen : null;
  const DRAFT_STORAGE_KEY = "dh_server_launcher_ui_draft_v1";

  const COMMAND_PREFIX_MAP = {
    summon: "summon",
    isThrall: "setisthrall",
    spellCharge: "setspellcharge",
    blizzardIntensity: "setblizzardintensity",
    slomo: "slomo",
    god: "god",
    endMatch: "endmatch",
  };

  const state = {
    activeTab: "console",
    config: null,
    configDraft: null,
    autoRestartWatchdog: true,
    serverExeExists: false,
    legacyFridaDllDetected: false,
    consoleCatalog: null,
    consoleDraft: null,
    maps: [],
    defaultMods: [],
    clientRequiredMods: [],
    practicalMods: [],
    funMods: [],
    conflictPairs: [],
    selectedMapId: "Departure_Persistent",
    selectedContentNumbers: new Set(),
    selectedOptionNumbers: new Set(),
    serverStatus: null,
    consoleLogs: [],
    serverLogs: [],
    syncTimer: null,
    planSyncTimer: null,
    submittingConsole: false,
    licenseText: null,
  };

  // ==================== Reload, Context-Menu & Header Dblclick Hardening ====================
  document.addEventListener(
    "contextmenu",
    function (event) {
      event.preventDefault();
    },
    { capture: true }
  );

  // Block double-click maximize on custom titlebar/header while keeping single-click drag intact
  window.addEventListener(
    "dblclick",
    function (event) {
      const target = event.target instanceof Element ? event.target : null;
      if (target && target.closest(".app-header, .neon-ribbon, [data-tauri-drag-region]")) {
        event.preventDefault();
        event.stopPropagation();
      }
    },
    { capture: true }
  );

  window.addEventListener(
    "mousedown",
    function (event) {
      const target = event.target instanceof Element ? event.target : null;
      if (
        event.detail >= 2 &&
        target &&
        target.closest(".app-header, .neon-ribbon, [data-tauri-drag-region]")
      ) {
        event.preventDefault();
        event.stopPropagation();
      }
    },
    { capture: true }
  );

  window.addEventListener(
    "keydown",
    function (event) {
      const key = (event.key || "").toLowerCase();
      const ctrlOrMeta = event.ctrlKey || event.metaKey;
      const isReloadShortcut =
        key === "f5" ||
        (ctrlOrMeta && key === "r") ||
        (ctrlOrMeta && event.shiftKey && key === "r");
      const isBrowserNavOrDevtools =
        key === "f12" ||
        (ctrlOrMeta && event.shiftKey && (key === "i" || key === "j" || key === "c")) ||
        (ctrlOrMeta && key === "u") ||
        (event.altKey && (key === "arrowleft" || key === "arrowright"));
      if (isReloadShortcut || isBrowserNavOrDevtools) {
        event.preventDefault();
        event.stopPropagation();
      }
    },
    { capture: true }
  );

  function byId(id) {
    return document.getElementById(id);
  }

  function showToast(message, type) {
    const container = byId("toastContainer");
    if (!container) return;
    const toast = document.createElement("div");
    toast.className = "toast " + (type || "success");

    const msgSpan = document.createElement("span");
    msgSpan.className = "toast-msg";
    msgSpan.textContent = String(message || "");

    const closeBtn = document.createElement("button");
    closeBtn.type = "button";
    closeBtn.className = "toast-close";
    closeBtn.textContent = "×";
    closeBtn.addEventListener("click", function () {
      if (toast.parentNode) toast.parentNode.removeChild(toast);
    });

    toast.appendChild(msgSpan);
    toast.appendChild(closeBtn);
    container.appendChild(toast);

    window.setTimeout(function () {
      if (toast.parentNode) toast.parentNode.removeChild(toast);
    }, 4200);
  }

  async function callBackend(cmd, args) {
    if (!invoke) {
      throw new Error("Tauri IPC が初期化されていません");
    }
    return await invoke(cmd, args || {});
  }

  function extractFileName(pathStr) {
    if (!pathStr) return "";
    const parts = pathStr.replace(/\\/g, "/").split("/");
    return parts[parts.length - 1] || "";
  }

  // ==================== Custom Angular Neo-Brutalist Dropdown Portal ====================
  const nbSelect = (function () {
    const widgetMap = new WeakMap();
    const allSelects = [];
    let activeSelect = null;
    let activeVisibleIdx = -1;
    let visibleOptionIndices = [];
    let portalReady = false;
    let portalWrap = null;
    let portalSearch = null;
    let portalSearchInput = null;
    let portalCount = null;
    let portalClear = null;
    let portalList = null;

    function ensurePortalDom() {
      if (portalReady) return;
      portalReady = true;

      portalWrap = document.createElement("div");
      portalWrap.className = "nb-portal-wrap";
      portalWrap.setAttribute("role", "dialog");
      portalWrap.setAttribute("aria-modal", "false");

      const portalBorder = document.createElement("div");
      portalBorder.className = "nb-portal-border";

      const portalMenu = document.createElement("div");
      portalMenu.className = "nb-portal-menu";

      portalSearch = document.createElement("div");
      portalSearch.className = "nb-portal-search";

      portalSearchInput = document.createElement("input");
      portalSearchInput.type = "text";
      portalSearchInput.className = "nb-portal-search-input";
      portalSearchInput.placeholder = "アイテムを検索 (日本語 / Blueprint名)...";

      portalCount = document.createElement("span");
      portalCount.className = "nb-portal-count";

      portalClear = document.createElement("button");
      portalClear.type = "button";
      portalClear.className = "nb-portal-clear";
      portalClear.setAttribute("aria-label", "検索クリア");
      portalClear.textContent = "×";

      portalSearch.appendChild(portalSearchInput);
      portalSearch.appendChild(portalCount);
      portalSearch.appendChild(portalClear);

      portalList = document.createElement("ul");
      portalList.className = "nb-portal-list";
      portalList.setAttribute("role", "listbox");

      portalMenu.appendChild(portalSearch);
      portalMenu.appendChild(portalList);
      portalBorder.appendChild(portalMenu);
      portalWrap.appendChild(portalBorder);
      document.body.appendChild(portalWrap);

      portalSearchInput.addEventListener("input", function () {
        if (!activeSelect) return;
        const filterEl = byId("conItemFilter");
        if (activeSelect.id === "conItemSelect" && filterEl) {
          filterEl.value = portalSearchInput.value;
          scheduleDraftSync(false);
        }
        renderPortalOptions(activeSelect, portalSearchInput.value);
        positionPortal(activeSelect);
      });

      portalSearchInput.addEventListener("keydown", function (event) {
        if (event.key === "ArrowDown") {
          event.preventDefault();
          moveActive(1);
        } else if (event.key === "ArrowUp") {
          event.preventDefault();
          moveActive(-1);
        } else if (event.key === "Enter") {
          event.preventDefault();
          commitActive();
        } else if (event.key === "Escape") {
          event.preventDefault();
          close(true);
        }
      });

      portalClear.addEventListener("click", function (event) {
        event.preventDefault();
        event.stopPropagation();
        portalSearchInput.value = "";
        if (activeSelect && activeSelect.id === "conItemSelect") {
          const filterEl = byId("conItemFilter");
          if (filterEl) filterEl.value = "";
          scheduleDraftSync(false);
        }
        if (activeSelect) {
          renderPortalOptions(activeSelect, "");
          positionPortal(activeSelect);
        }
        portalSearchInput.focus();
      });

      document.addEventListener("click", function (event) {
        if (!activeSelect) return;
        const target = event.target;
        if (target instanceof Node && portalWrap.contains(target)) return;
        const entry = widgetMap.get(activeSelect);
        if (entry && target instanceof Node && entry.root.contains(target)) return;
        close(false);
      });

      document.addEventListener("keydown", function (event) {
        if (!activeSelect) return;
        if (document.activeElement === portalSearchInput) return;
        if (event.key === "Escape") {
          event.preventDefault();
          close(true);
        } else if (event.key === "ArrowDown") {
          event.preventDefault();
          moveActive(1);
        } else if (event.key === "ArrowUp") {
          event.preventDefault();
          moveActive(-1);
        } else if (event.key === "Home") {
          event.preventDefault();
          setActiveVisible(0);
        } else if (event.key === "End") {
          event.preventDefault();
          setActiveVisible(visibleOptionIndices.length - 1);
        } else if (event.key === "Enter") {
          event.preventDefault();
          commitActive();
        } else if (event.key === "Tab") {
          close(false);
        }
      });

      window.addEventListener("resize", function () {
        if (activeSelect) positionPortal(activeSelect);
      });

      window.addEventListener(
        "scroll",
        function (event) {
          if (!activeSelect) return;
          if (event.target instanceof Node && portalWrap.contains(event.target)) return;
          positionPortal(activeSelect);
        },
        true
      );
    }

    function getSelectedText(select) {
      const idx = select.selectedIndex;
      if (idx >= 0 && select.options[idx]) {
        return select.options[idx].textContent || "";
      }
      return select.options[0] ? select.options[0].textContent || "" : "";
    }

    function refresh(select) {
      if (!select) return;
      const entry = widgetMap.get(select);
      if (!entry) return;
      entry.valueEl.textContent = getSelectedText(select);
      entry.trigger.disabled = Boolean(select.disabled);
    }

    function refreshAll() {
      allSelects.forEach(refresh);
    }

    function setVisible(select, visible) {
      if (!select) return;
      const entry = widgetMap.get(select);
      if (!entry) return;
      entry.root.style.display = visible ? "" : "none";
      if (!visible && activeSelect === select) {
        close(false);
      }
    }

    function setActiveVisible(visIdx) {
      const items = portalList.querySelectorAll(".nb-portal-option");
      if (items.length === 0) {
        activeVisibleIdx = -1;
        return;
      }
      const clamped = Math.max(0, Math.min(items.length - 1, visIdx));
      activeVisibleIdx = clamped;
      items.forEach(function (item, idx) {
        if (idx === clamped) {
          item.classList.add("is-active");
          item.scrollIntoView({ block: "nearest" });
        } else {
          item.classList.remove("is-active");
        }
      });
    }

    function moveActive(delta) {
      const count = visibleOptionIndices.length;
      if (count === 0) return;
      const next = activeVisibleIdx < 0 ? 0 : (activeVisibleIdx + delta + count) % count;
      setActiveVisible(next);
    }

    function commitOptionIndex(optionIdx) {
      if (!activeSelect) return;
      const target = activeSelect;
      if (optionIdx >= 0 && optionIdx < target.options.length) {
        const changed = target.selectedIndex !== optionIdx;
        target.selectedIndex = optionIdx;
        refresh(target);
        close(true);
        if (changed) {
          target.dispatchEvent(new Event("change", { bubbles: true }));
        }
      } else {
        close(true);
      }
    }

    function commitActive() {
      if (activeVisibleIdx >= 0 && activeVisibleIdx < visibleOptionIndices.length) {
        commitOptionIndex(visibleOptionIndices[activeVisibleIdx]);
      } else {
        close(true);
      }
    }

    function renderPortalOptions(select, rawQuery) {
      const query = (rawQuery || "").trim().toLowerCase();
      const total = select.options.length;
      const selectedIdx = select.selectedIndex;

      portalList.textContent = "";
      visibleOptionIndices = [];
      let selectedVisiblePos = -1;

      for (let i = 0; i < total; i += 1) {
        const opt = select.options[i];
        const label = opt.textContent || "";
        const val = opt.value || "";
        if (
          query &&
          label.toLowerCase().indexOf(query) === -1 &&
          val.toLowerCase().indexOf(query) === -1
        ) {
          continue;
        }
        const visPos = visibleOptionIndices.length;
        visibleOptionIndices.push(i);
        if (i === selectedIdx) selectedVisiblePos = visPos;

        const li = document.createElement("li");
        li.className = "nb-portal-option" + (i === selectedIdx ? " is-selected" : "");
        li.setAttribute("role", "option");
        li.setAttribute("aria-selected", i === selectedIdx ? "true" : "false");

        const textSpan = document.createElement("span");
        textSpan.textContent = label;

        const mark = document.createElement("span");
        mark.className = "nb-opt-mark";
        mark.setAttribute("aria-hidden", "true");

        li.appendChild(textSpan);
        li.appendChild(mark);

        li.addEventListener("mouseenter", function () {
          setActiveVisible(visPos);
        });

        li.addEventListener("click", function (event) {
          event.preventDefault();
          event.stopPropagation();
          commitOptionIndex(i);
        });

        portalList.appendChild(li);
      }

      if (select.getAttribute("data-searchable") === "true") {
        portalCount.textContent = visibleOptionIndices.length + " / " + total;
        portalClear.classList.toggle("is-visible", Boolean(rawQuery));
      }

      if (visibleOptionIndices.length === 0) {
        const empty = document.createElement("li");
        empty.className = "nb-portal-empty";
        empty.textContent = "一致する項目がありません";
        portalList.appendChild(empty);
        activeVisibleIdx = -1;
      } else {
        setActiveVisible(selectedVisiblePos >= 0 ? selectedVisiblePos : 0);
      }
    }

    function positionPortal(select) {
      const entry = widgetMap.get(select);
      if (!entry) return;
      const rect = entry.trigger.getBoundingClientRect();
      if (rect.width === 0 && rect.height === 0) {
        close(false);
        return;
      }
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      const margin = 12;
      const gap = 6;
      const maxListH = 256;
      const searchOverhead = select.getAttribute("data-searchable") === "true" ? 50 : 0;

      const width = Math.min(Math.max(rect.width, 220), vw - margin * 2);
      const left = Math.max(margin, Math.min(rect.left, vw - width - margin));
      portalWrap.style.width = Math.round(width) + "px";
      portalWrap.style.left = Math.round(left) + "px";

      const spaceBelow = vh - rect.bottom - margin - gap;
      const spaceAbove = rect.top - margin - gap;
      const desiredTotalH = Math.min(portalList.scrollHeight + 14, maxListH) + searchOverhead + 10;

      if (spaceBelow < Math.min(desiredTotalH, 210) && spaceAbove > spaceBelow) {
        const listMaxH = Math.max(110, Math.min(maxListH, spaceAbove - searchOverhead - 16));
        portalList.style.maxHeight = Math.floor(listMaxH) + "px";
        const actualH = portalWrap.offsetHeight || Math.min(desiredTotalH, listMaxH + searchOverhead + 12);
        portalWrap.style.top = Math.max(margin, Math.round(rect.top - actualH - gap)) + "px";
      } else {
        const listMaxH = Math.max(110, Math.min(maxListH, spaceBelow - searchOverhead - 16));
        portalList.style.maxHeight = Math.floor(listMaxH) + "px";
        portalWrap.style.top = Math.round(rect.bottom + gap) + "px";
      }
    }

    function open(select) {
      ensurePortalDom();
      const entry = widgetMap.get(select);
      if (!entry || entry.trigger.disabled) return;
      if (activeSelect && activeSelect !== select) {
        close(false);
      }

      activeSelect = select;
      entry.root.classList.add("is-open");
      entry.trigger.setAttribute("aria-expanded", "true");

      const searchable = select.getAttribute("data-searchable") === "true";
      portalSearch.classList.toggle("is-visible", searchable);
      let initialQuery = "";
      if (searchable) {
        const filterEl = select.id === "conItemSelect" ? byId("conItemFilter") : null;
        initialQuery = filterEl ? filterEl.value || "" : "";
        portalSearchInput.value = initialQuery;
      } else {
        portalSearchInput.value = "";
      }

      renderPortalOptions(select, initialQuery);
      portalWrap.classList.add("is-open");
      positionPortal(select);

      if (searchable) {
        window.requestAnimationFrame(function () {
          if (activeSelect === select && portalSearchInput) {
            portalSearchInput.focus();
            portalSearchInput.select();
          }
        });
      }
    }

    function close(restoreFocus) {
      if (!activeSelect) return;
      const prev = activeSelect;
      const entry = widgetMap.get(prev);
      activeSelect = null;
      activeVisibleIdx = -1;
      visibleOptionIndices = [];

      if (entry) {
        entry.root.classList.remove("is-open");
        entry.trigger.setAttribute("aria-expanded", "false");
      }
      if (portalWrap) {
        portalWrap.classList.remove("is-open");
      }
      if (restoreFocus && entry && entry.trigger.isConnected) {
        entry.trigger.focus({ preventScroll: true });
      }
    }

    function enhanceOne(select) {
      if (!select || widgetMap.has(select)) {
        refresh(select);
        return;
      }
      ensurePortalDom();

      select.classList.add("native-select-hidden");
      select.setAttribute("tabindex", "-1");
      select.setAttribute("aria-hidden", "true");

      const root = document.createElement("div");
      root.className = "nb-select";
      root.dataset.nbFor = select.id || "";

      const shell = document.createElement("div");
      shell.className = "nb-select-shell";

      const trigger = document.createElement("button");
      trigger.type = "button";
      trigger.className = "nb-select-trigger";
      trigger.setAttribute("aria-haspopup", "listbox");
      trigger.setAttribute("aria-expanded", "false");

      const valueEl = document.createElement("span");
      valueEl.className = "nb-select-value";

      const chevron = document.createElement("span");
      chevron.className = "nb-select-chevron";
      chevron.setAttribute("aria-hidden", "true");
      chevron.innerHTML =
        '<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">' +
        '<path d="M2.5 5.25L8 11.25L13.5 5.25" stroke="currentColor" stroke-width="2.8" stroke-linecap="square" stroke-linejoin="miter"/>' +
        "</svg>";

      trigger.appendChild(valueEl);
      trigger.appendChild(chevron);
      shell.appendChild(trigger);
      root.appendChild(shell);

      const entry = { root: root, trigger: trigger, valueEl: valueEl, select: select };
      widgetMap.set(select, entry);
      allSelects.push(select);
      select.insertAdjacentElement("afterend", root);
      refresh(select);

      trigger.addEventListener("click", function (event) {
        event.preventDefault();
        event.stopPropagation();
        if (activeSelect === select) {
          close(false);
        } else {
          open(select);
        }
      });

      trigger.addEventListener("keydown", function (event) {
        if (
          event.key === "ArrowDown" ||
          event.key === "ArrowUp" ||
          event.key === "Enter" ||
          event.key === " "
        ) {
          event.preventDefault();
          event.stopPropagation();
          if (activeSelect !== select) {
            open(select);
          } else if (event.key === "ArrowDown") {
            moveActive(1);
          } else if (event.key === "ArrowUp") {
            moveActive(-1);
          } else {
            commitActive();
          }
        } else if (event.key === "Escape" && activeSelect === select) {
          event.preventDefault();
          event.stopPropagation();
          close(true);
        }
      });

      select.addEventListener("change", function () {
        refresh(select);
      });
    }

    function enhanceAll() {
      document.querySelectorAll("select.nb-select-native").forEach(enhanceOne);
      document.addEventListener("click", function (event) {
        const label = event.target instanceof Element ? event.target.closest("label[for]") : null;
        if (!label) return;
        const targetId = label.getAttribute("for");
        if (!targetId) return;
        const targetEl = byId(targetId);
        if (targetEl && targetEl.tagName === "SELECT" && widgetMap.has(targetEl)) {
          event.preventDefault();
          event.stopPropagation();
          if (activeSelect === targetEl) {
            close(false);
          } else {
            open(targetEl);
          }
        }
      });
    }

    return {
      enhanceAll: enhanceAll,
      refresh: refresh,
      refreshAll: refreshAll,
      setVisible: setVisible,
      close: close,
      getActiveSelect: function () { return activeSelect; },
    };
  })();

  function populateSelect(selectEl, items, selectedValue) {
    if (!selectEl) return;
    selectEl.textContent = "";
    items.forEach(function (item) {
      const opt = document.createElement("option");
      opt.value = item.value;
      opt.textContent = item.label;
      selectEl.appendChild(opt);
    });
    if (selectedValue !== undefined && selectedValue !== null) {
      const hasMatch = items.some(function (it) { return it.value === selectedValue; });
      if (hasMatch) {
        selectEl.value = selectedValue;
      } else if (items.length > 0) {
        selectEl.value = items[0].value;
      }
    }
    nbSelect.refresh(selectEl);
  }

  function buildUiDraftPayload() {
    const cfg = buildConfigFromForm();
    return {
      activeTab: state.activeTab,
      consoleDraft: readConsoleDraftFromForm(),
      configDraft: cfg,
      autoRestartWatchdog: Boolean(byId("cfgWatchdog").checked),
      matchDraft: {
        mapId: state.selectedMapId,
        settings: readMatchSettingsFromForm(),
        selectedContentNumbers: Array.from(state.selectedContentNumbers).sort(function (a, b) { return a - b; }),
        selectedOptionNumbers: Array.from(state.selectedOptionNumbers).sort(function (a, b) { return a - b; }),
      },
    };
  }

  function persistLocalDraft(draft) {
    try {
      window.localStorage.setItem(DRAFT_STORAGE_KEY, JSON.stringify(draft));
    } catch (_) {
      // Ignore storage quota / restriction errors
    }
  }

  function scheduleDraftSync(immediate) {
    const draft = buildUiDraftPayload();
    state.consoleDraft = draft.consoleDraft;
    state.configDraft = draft.configDraft;
    persistLocalDraft(draft);

    if (state.syncTimer !== null) {
      window.clearTimeout(state.syncTimer);
      state.syncTimer = null;
    }

    const push = function () {
      state.syncTimer = null;
      if (!invoke) return;
      invoke("sync_ui_draft", { draft: buildUiDraftPayload() }).catch(function () {});
    };

    if (immediate) {
      push();
    } else {
      state.syncTimer = window.setTimeout(push, 120);
    }
  }

  function scheduleNextRoundAutoSync() {
    if (!invoke) return;
    const phase = state.serverStatus ? state.serverStatus.phase : "stopped";
    if (!phase || phase === "stopped") return;

    if (state.planSyncTimer !== null && state.planSyncTimer !== undefined) {
      window.clearTimeout(state.planSyncTimer);
      state.planSyncTimer = null;
    }

    state.planSyncTimer = window.setTimeout(function () {
      state.planSyncTimer = null;
      const curPhase = state.serverStatus ? state.serverStatus.phase : "stopped";
      if (!curPhase || curPhase === "stopped") return;
      invoke("update_next_round_plan", { request: buildLaunchRequest() }).catch(function () {});
    }, 300);
  }

  function switchTab(tabName, skipSync) {
    nbSelect.close(false);
    const validTabs = ["console", "mods", "monitor", "settings"];
    const target = validTabs.indexOf(tabName) !== -1 ? tabName : "console";
    state.activeTab = target;
    document.querySelectorAll(".nav-tab-wrap").forEach(function (wrap) {
      wrap.classList.toggle("is-active", wrap.getAttribute("data-tab-wrap") === target);
    });
    document.querySelectorAll(".tab-panel").forEach(function (panel) {
      panel.classList.toggle("is-active", panel.id === "tab-" + target);
    });
    if (!skipSync) {
      scheduleDraftSync(true);
    }
  }

  // ==================== Tab 01: In-App Admin Console ====================
  function readConsoleDraftFromForm() {
    const cur = state.consoleDraft || {};
    return {
      operation: cur.operation || "console_command",
      targetMode: cur.targetMode || "role",
      targetRole: byId("conTargetRole").value || cur.targetRole || "Captain",
      customTarget: byId("conCustomTarget").value || "",
      autoBroadcast: Boolean(byId("conAutoBroadcast").checked),
      consoleOrder: byId("conCommandType").value || "summon",
      summonTarget: byId("conSummon").value || "BP_Prey_Rat_C",
      isThrall: byId("conIsThrall").value || "1",
      spellCharge: byId("conSpellCharge").value || "0.37",
      blizzardIntensity: byId("conBlizzard").value || "0.15",
      slomo: byId("conSlomo").value || "2",
      god: byId("conGod").value || "1",
      endMatch: byId("conEndMatch").value || "1",
      rawCommand: byId("conRawCommand").value || "",
      statusProp: byId("conStatusProp").value || "bSpiritWalking",
      statusToggle: byId("conStatusToggle").value || "1",
      statusNumber: byId("conStatusNumber").value || "100",
      itemFilter: byId("conItemFilter").value || "",
      itemPath: byId("conItemSelect").value || cur.itemPath || "",
      itemCount: byId("conItemCount").value || "1",
      storageLimit: byId("conStorageLimit").value || "10",
      chatBroadcast: Boolean(byId("conChatBroadcast").checked),
      chatMessage: byId("conChatMessage").value || "",
    };
  }

  function hydrateConsoleCatalogAndDraft() {
    const cat = state.consoleCatalog;
    const draft = state.consoleDraft;
    if (!cat || !draft) return;

    populateSelect(byId("conTargetRole"), cat.roles, draft.targetRole);
    populateSelect(byId("conCommandType"), cat.commandTypes, draft.consoleOrder);
    populateSelect(byId("conSummon"), cat.summons, draft.summonTarget);
    populateSelect(byId("conSpellCharge"), cat.spellCharges, draft.spellCharge);
    populateSelect(byId("conBlizzard"), cat.blizzardIntensities, draft.blizzardIntensity);
    populateSelect(byId("conSlomo"), cat.slomos, draft.slomo);
    populateSelect(
      byId("conStatusProp"),
      cat.statuses.map(function (s) { return { value: s.value, label: s.label }; }),
      draft.statusProp
    );

    byId("conCustomTarget").value = draft.customTarget || "";
    byId("conAutoBroadcast").checked = Boolean(draft.autoBroadcast);
    byId("conIsThrall").value = draft.isThrall || "1";
    byId("conGod").value = draft.god || "1";
    byId("conEndMatch").value = draft.endMatch || "1";
    byId("conRawCommand").value = draft.rawCommand || "";
    byId("conStatusToggle").value = draft.statusToggle || "1";
    byId("conStatusNumber").value = draft.statusNumber || "100";
    byId("conItemFilter").value = draft.itemFilter || "";
    byId("conItemCount").value = draft.itemCount || "1";
    byId("conStorageLimit").value = draft.storageLimit || "10";
    byId("conChatBroadcast").checked = Boolean(draft.chatBroadcast);
    byId("conChatMessage").value = draft.chatMessage || "";

    populateSelect(byId("conItemSelect"), cat.items, draft.itemPath);
    nbSelect.refreshAll();
    renderItemCountPresets();
    renderStoragePresets();
    renderConsoleVisibility();
  }

  function renderItemCountPresets() {
    const cat = state.consoleCatalog;
    const box = byId("conItemCountPresets");
    if (!cat || !box) return;
    box.textContent = "";
    const curVal = String(byId("conItemCount").value || "");
    cat.itemCounts.forEach(function (cnt) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "preset-btn" + (curVal === String(cnt) ? " is-active" : "");
      btn.textContent = "x" + cnt;
      btn.addEventListener("click", function () {
        byId("conItemCount").value = String(cnt);
        renderItemCountPresets();
        scheduleDraftSync(false);
      });
      box.appendChild(btn);
    });
  }

  const STORAGE_PRESET_SHORT_LABELS = {
    "10": "10 (通常)",
    "20": "20 (2倍)",
    "30": "30 (3倍)",
  };

  function renderStoragePresets() {
    const cat = state.consoleCatalog;
    const box = byId("conStoragePresets");
    if (!cat || !box) return;
    box.textContent = "";
    const curVal = String(byId("conStorageLimit").value || "");
    cat.storages.forEach(function (st) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "preset-btn" + (curVal === st.value ? " is-active" : "");
      btn.textContent = STORAGE_PRESET_SHORT_LABELS[st.value] || (st.value + " (" + st.label + ")");
      btn.title = st.value + " (" + st.label + ")";
      btn.addEventListener("click", function () {
        byId("conStorageLimit").value = st.value;
        renderStoragePresets();
        scheduleDraftSync(false);
      });
      box.appendChild(btn);
    });
  }

  function renderConsoleVisibility() {
    nbSelect.close(false);
    const draft = state.consoleDraft;
    if (!draft) return;
    const op = draft.operation || "console_command";

    document.querySelectorAll("[data-op-wrap]").forEach(function (wrap) {
      wrap.classList.toggle("is-active", wrap.getAttribute("data-op-wrap") === op);
    });

    const isChatBroadcast = op === "send_thrall_msg" && Boolean(byId("conChatBroadcast").checked);
    byId("consoleTargetGroup").style.display = isChatBroadcast ? "none" : "flex";

    const useCustom = draft.targetMode === "custom";
    byId("btnTargetRole").classList.toggle("is-active", !useCustom);
    byId("btnTargetCustom").classList.toggle("is-active", useCustom);
    nbSelect.setVisible(byId("conTargetRole"), !useCustom);
    byId("conCustomTarget").style.display = useCustom ? "block" : "none";

    byId("conPanelCommand").style.display = op === "console_command" ? "flex" : "none";
    byId("conPanelStatus").style.display = op === "set_human" ? "flex" : "none";
    byId("conPanelItem").style.display = op === "add_item" ? "flex" : "none";
    byId("conPanelStorage").style.display = op === "set_storage" ? "flex" : "none";
    byId("conPanelChat").style.display = op === "send_thrall_msg" ? "flex" : "none";
    byId("conPanelKick").style.display = op === "kick_player" ? "flex" : "none";
    byId("conAutoBroadcastWrap").style.display = op === "send_thrall_msg" ? "none" : "flex";

    // Command sub-fields
    const cmdType = byId("conCommandType").value || "summon";
    byId("conSubSummon").style.display = cmdType === "summon" ? "flex" : "none";
    byId("conSubThrall").style.display = cmdType === "isThrall" ? "flex" : "none";
    byId("conSubSpellCharge").style.display = cmdType === "spellCharge" ? "flex" : "none";
    byId("conSubBlizzard").style.display = cmdType === "blizzardIntensity" ? "flex" : "none";
    byId("conSubSlomo").style.display = cmdType === "slomo" ? "flex" : "none";
    byId("conSubGod").style.display = cmdType === "god" ? "flex" : "none";
    byId("conSubEndMatch").style.display = cmdType === "endMatch" ? "flex" : "none";
    byId("conSubRaw").style.display = cmdType === "raw" ? "flex" : "none";

    // Status toggle vs number
    const statusProp = byId("conStatusProp").value || "bSpiritWalking";
    const statusMeta =
      state.consoleCatalog && state.consoleCatalog.statuses
        ? state.consoleCatalog.statuses.find(function (s) { return s.value === statusProp; })
        : null;
    const isToggle = statusMeta ? Boolean(statusMeta.isToggle) : false;
    byId("conStatusToggleWrap").style.display = isToggle ? "flex" : "none";
    byId("conStatusNumWrap").style.display = isToggle ? "none" : "flex";
  }

  function buildConsoleActionRequest() {
    const draft = readConsoleDraftFromForm();
    const targetPlayer =
      draft.targetMode === "custom" ? draft.customTarget.trim() : draft.targetRole;

    switch (draft.operation) {
      case "console_command": {
        let fullCommand = "";
        if (draft.consoleOrder === "raw") {
          fullCommand = draft.rawCommand.trim();
        } else {
          const prefix = COMMAND_PREFIX_MAP[draft.consoleOrder] || draft.consoleOrder;
          let argVal = "";
          if (draft.consoleOrder === "summon") argVal = draft.summonTarget;
          else if (draft.consoleOrder === "isThrall") argVal = draft.isThrall;
          else if (draft.consoleOrder === "spellCharge") argVal = draft.spellCharge;
          else if (draft.consoleOrder === "blizzardIntensity") argVal = draft.blizzardIntensity;
          else if (draft.consoleOrder === "slomo") argVal = draft.slomo;
          else if (draft.consoleOrder === "god") argVal = draft.god;
          else if (draft.consoleOrder === "endMatch") argVal = draft.endMatch;
          fullCommand = prefix + " " + argVal;
        }
        return {
          action: "console_command",
          playerName: targetPlayer,
          command: fullCommand,
          autoBroadcast: draft.autoBroadcast,
        };
      }
      case "set_human": {
        const statusMeta =
          state.consoleCatalog && state.consoleCatalog.statuses
            ? state.consoleCatalog.statuses.find(function (s) { return s.value === draft.statusProp; })
            : null;
        const isToggle = statusMeta ? Boolean(statusMeta.isToggle) : false;
        return {
          action: "set_human",
          playerName: targetPlayer,
          propName: draft.statusProp,
          value: isToggle ? draft.statusToggle : draft.statusNumber,
          autoBroadcast: draft.autoBroadcast,
        };
      }
      case "add_item":
        return {
          action: "add_item",
          playerName: targetPlayer,
          itemName: draft.itemPath,
          count: draft.itemCount,
          autoBroadcast: draft.autoBroadcast,
        };
      case "set_storage":
        return {
          action: "set_storage",
          playerName: targetPlayer,
          limit: draft.storageLimit,
          autoBroadcast: draft.autoBroadcast,
        };
      case "send_thrall_msg":
        return {
          action: "send_thrall_msg",
          playerName: targetPlayer,
          msg: draft.chatMessage,
          broadcast: draft.chatBroadcast,
        };
      case "kick_player":
        return {
          action: "kick_player",
          playerName: targetPlayer,
          autoBroadcast: draft.autoBroadcast,
        };
      default:
        return null;
    }
  }

  // ==================== Tab 02: Match & MOD Setup ====================
  function allServerOnlyMods() {
    return state.practicalMods.concat(state.funMods);
  }

  function findServerModById(modId) {
    return allServerOnlyMods().find(function (m) { return m.id === modId; }) || null;
  }

  function findServerModByNumber(num) {
    return allServerOnlyMods().find(function (m) { return m.number === num; }) || null;
  }

  function getActiveConflictsForMod(mod) {
    if (!mod.conflictsWith || mod.conflictsWith.length === 0) return [];
    const conflicting = [];
    state.selectedOptionNumbers.forEach(function (selNum) {
      const other = findServerModByNumber(selNum);
      if (other && other.id !== mod.id) {
        const forward = mod.conflictsWith.indexOf(other.id) !== -1;
        const reverse = other.conflictsWith && other.conflictsWith.indexOf(mod.id) !== -1;
        if (forward || reverse) {
          conflicting.push(other);
        }
      }
    });
    return conflicting;
  }

  function readMatchSettingsFromForm() {
    return {
      maxPlayers: parseInt(byId("ruleMaxPlayers").value, 10) || 8,
      daysBeforeBlizzard: parseInt(byId("ruleDaysBlizzard").value, 10) || 3,
      dayMinutes: parseInt(byId("ruleDayMinutes").value, 10) || 9,
      predatorDamage: parseFloat(byId("rulePredatorDmg").value) || 1.0,
      coldIntensity: parseFloat(byId("ruleCold").value) || 1.0,
      hungerRate: parseFloat(byId("ruleHunger").value) || 1.0,
      coalBurnRate: parseFloat(byId("ruleCoal").value) || 1.0,
      thralls: parseInt(byId("ruleThralls").value, 10) || 0,
    };
  }

  function applyMatchSettingsToForm(settings) {
    if (!settings) return;
    byId("ruleMaxPlayers").value = String(settings.maxPlayers);
    byId("ruleDaysBlizzard").value = String(settings.daysBeforeBlizzard);
    byId("ruleDayMinutes").value = String(settings.dayMinutes);
    byId("rulePredatorDmg").value = String(settings.predatorDamage);
    byId("ruleCold").value = String(settings.coldIntensity);
    byId("ruleHunger").value = String(settings.hungerRate);
    byId("ruleCoal").value = String(settings.coalBurnRate);
    byId("ruleThralls").value = String(settings.thralls);
    renderUeArgPreview();
  }

  function renderUeArgPreview() {
    const s = readMatchSettingsFromForm();
    const preview =
      state.selectedMapId +
      "?maxplayers=" + s.maxPlayers +
      "?daysbeforeblizzard=" + s.daysBeforeBlizzard +
      "?dayminutes=" + s.dayMinutes +
      "?predatordamage=" + s.predatorDamage +
      "?coldintensity=" + s.coldIntensity +
      "?hungerrate=" + s.hungerRate +
      "?coalburnrate=" + s.coalBurnRate +
      "?thralls=" + s.thralls;
    byId("ueArgPreview").textContent = "UE起動引数: " + preview;
  }

  function renderMaps() {
    const grid = byId("mapGrid");
    grid.textContent = "";
    state.maps.forEach(function (mapOpt) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "map-tile" + (state.selectedMapId === mapOpt.id ? " is-selected" : "");

      const textWrap = document.createElement("div");
      const nameEl = document.createElement("div");
      nameEl.className = "map-tile-name";
      nameEl.textContent = mapOpt.label;
      const idEl = document.createElement("div");
      idEl.className = "map-tile-id";
      idEl.textContent = mapOpt.id;
      textWrap.appendChild(nameEl);
      textWrap.appendChild(idEl);

      const diamond = document.createElement("span");
      diamond.className = "map-diamond";

      btn.appendChild(textWrap);
      btn.appendChild(diamond);
      btn.addEventListener("click", function () {
        state.selectedMapId = mapOpt.id;
        renderMaps();
        renderUeArgPreview();
        scheduleDraftSync(true);
        scheduleNextRoundAutoSync();
      });
      grid.appendChild(btn);
    });
  }

  function renderDefaultMods() {
    const strip = byId("defaultModsStrip");
    strip.textContent = "";
    state.defaultMods.forEach(function (m) {
      const chip = document.createElement("span");
      chip.className = "chip dark";
      chip.textContent = m.label + " (" + m.fileName + ")";
      strip.appendChild(chip);
    });
  }

  function buildModCard(mod, options) {
    const isClientReq = Boolean(options.isClientReq);
    const isFun = Boolean(options.isFun);
    const isSelected = isClientReq
      ? state.selectedContentNumbers.has(mod.number)
      : state.selectedOptionNumbers.has(mod.number);
    const activeConflicts = isClientReq ? [] : getActiveConflictsForMod(mod);

    const card = document.createElement("button");
    card.type = "button";
    let cls = "mod-card";
    if (isClientReq) {
      cls += " is-client-req";
    } else {
      cls += " is-server-only";
    }
    if (isFun) cls += " is-fun";
    if (isSelected) cls += " is-selected";
    if (activeConflicts.length > 0 && !isSelected) cls += " is-conflicted";
    card.className = cls;

    const titleWrap = document.createElement("div");
    titleWrap.className = "mod-card-title-wrap";

    const numSpan = document.createElement("span");
    numSpan.className = "mod-num";
    numSpan.textContent = isClientReq ? "C" + mod.number : "#" + mod.number;

    const titleSpan = document.createElement("span");
    titleSpan.className = "mod-title";
    titleSpan.textContent = mod.label;

    titleWrap.appendChild(numSpan);
    titleWrap.appendChild(titleSpan);

    const check = document.createElement("span");
    check.className = "mod-check";
    check.textContent = isSelected ? "✓" : "";

    if (!isClientReq) {
      // Compact uniform-height single-row server-only MOD card
      const rightWrap = document.createElement("div");
      rightWrap.className = "mod-card-right";

      const tooltipLines = [mod.label];

      if (mod.conflictsWith && mod.conflictsWith.length > 0) {
        const conflictNames = mod.conflictsWith.map(function (cid) {
          const found = findServerModById(cid);
          return found ? found.label : cid;
        });
        tooltipLines.push("排他対象: " + conflictNames.join(" / "));
        if (activeConflicts.length > 0 && !isSelected) {
          tooltipLines.push("クリックすると選択中の排他MODを自動解除して切替えます");
        }
        const confBadge = document.createElement("span");
        confBadge.className =
          "mod-inline-badge is-conflict" +
          (activeConflicts.length > 0 && !isSelected ? " is-active-conflict" : "");
        confBadge.textContent = "排他";
        rightWrap.appendChild(confBadge);
      }

      if (mod.description) {
        tooltipLines.push(mod.description);
      }
      card.title = tooltipLines.join("\n");

      rightWrap.appendChild(check);
      card.appendChild(titleWrap);
      card.appendChild(rightWrap);
    } else {
      const top = document.createElement("div");
      top.className = "mod-card-top";
      top.appendChild(titleWrap);
      top.appendChild(check);
      card.appendChild(top);

      const meta = document.createElement("div");
      meta.className = "mod-meta";

      if (mod.requirementBadge) {
        const reqChip = document.createElement("span");
        reqChip.className = mod.requirementBadge.indexOf("DLL") !== -1 ? "chip purple" : "chip magenta";
        reqChip.textContent = mod.requirementBadge;
        meta.appendChild(reqChip);
      }

      if (mod.requirementDetail) {
        const detChip = document.createElement("span");
        detChip.className = "chip";
        detChip.textContent = mod.requirementDetail;
        meta.appendChild(detChip);
      }

      if (meta.childNodes.length > 0) {
        card.appendChild(meta);
      }

      if (mod.description) {
        const descEl = document.createElement("div");
        descEl.className = "mod-desc";
        descEl.textContent = mod.description;
        card.appendChild(descEl);
      }
    }

    card.addEventListener("click", function () {
      if (isClientReq) {
        if (state.selectedContentNumbers.has(mod.number)) {
          state.selectedContentNumbers.delete(mod.number);
        } else {
          state.selectedContentNumbers.add(mod.number);
        }
      } else {
        if (state.selectedOptionNumbers.has(mod.number)) {
          state.selectedOptionNumbers.delete(mod.number);
        } else {
          const conflicts = getActiveConflictsForMod(mod);
          if (conflicts.length > 0) {
            conflicts.forEach(function (c) {
              state.selectedOptionNumbers.delete(c.number);
            });
            const removedNames = conflicts.map(function (c) { return "「" + c.label + "」"; }).join("・");
            showToast("排他ルールにより " + removedNames + " を解除し、「" + mod.label + "」を選択しました", "warn");
          }
          state.selectedOptionNumbers.add(mod.number);
        }
      }
      renderModGrids();
      scheduleDraftSync(true);
      scheduleNextRoundAutoSync();
    });

    return card;
  }

  function renderModGrids() {
    const clientGrid = byId("clientRequiredModGrid");
    clientGrid.textContent = "";
    state.clientRequiredMods.forEach(function (mod) {
      clientGrid.appendChild(buildModCard(mod, { isClientReq: true, isFun: false }));
    });

    const practicalGrid = byId("practicalModGrid");
    practicalGrid.textContent = "";
    let practicalSelected = 0;
    state.practicalMods.forEach(function (mod) {
      if (state.selectedOptionNumbers.has(mod.number)) practicalSelected += 1;
      practicalGrid.appendChild(buildModCard(mod, { isClientReq: false, isFun: false }));
    });

    const funGrid = byId("funModGrid");
    funGrid.textContent = "";
    let funSelected = 0;
    state.funMods.forEach(function (mod) {
      if (state.selectedOptionNumbers.has(mod.number)) funSelected += 1;
      funGrid.appendChild(buildModCard(mod, { isClientReq: false, isFun: true }));
    });

    const clientCount = state.selectedContentNumbers.size;
    const totalCount = state.defaultMods.length + clientCount + practicalSelected + funSelected;

    byId("clientModCountChip").textContent = clientCount + " 件選択中";
    byId("practicalModCountChip").textContent = practicalSelected + " 件選択中";
    byId("funModCountChip").textContent = funSelected + " 件選択中";
    byId("tabModBadge").textContent = "MOD " + totalCount;

    const summary = byId("modSummaryBar");
    summary.textContent = "";
    [
      { cls: "chip info", text: "自動注入 " + state.defaultMods.length },
      { cls: "chip purple", text: "クライアント必須 " + clientCount },
      { cls: "chip warn", text: "サーバーのみ " + (practicalSelected + funSelected) },
    ].forEach(function (c) {
      const span = document.createElement("span");
      span.className = c.cls;
      span.textContent = c.text;
      summary.appendChild(span);
    });
  }

  // ==================== Tab 03: Server Monitor ====================
  function phaseLabel(phase) {
    switch (phase) {
      case "launching": return "起動・注入中";
      case "running": return "稼働中";
      case "countdown": return "再起動待機中";
      case "restarting": return "再起動中";
      case "stopping": return "停止処理中";
      default: return "停止中";
    }
  }

  function phasePillClass(phase) {
    switch (phase) {
      case "launching": return "is-launching";
      case "running": return "is-running";
      case "countdown": return "is-countdown";
      case "restarting": return "is-restarting";
      case "stopping": return "is-stopping";
      default: return "is-stopped";
    }
  }

  function renderServerMonitor() {
    const st = state.serverStatus;
    if (!st) return;

    const phase = st.phase || "stopped";
    const isStopped = phase === "stopped";
    const isActive = !isStopped;

    const isCountdown = phase === "countdown" && typeof st.countdownSeconds === "number";

    byId("hdrServerPill").className = "status-pill " + phasePillClass(phase);
    byId("hdrServerText").textContent = isCountdown
      ? "サーバー: 自動再起動 " + st.countdownSeconds + "秒"
      : "サーバー: " + phaseLabel(phase) + (st.activePort && isActive ? " :" + st.activePort : "");

    const monBadge = byId("tabMonBadge");
    if (isCountdown) {
      monBadge.textContent = st.countdownSeconds + "s";
      monBadge.classList.add("is-countdown");
    } else {
      monBadge.textContent = isActive ? (st.activePort ? ":" + st.activePort : "RUN") : "IDLE";
      monBadge.classList.remove("is-countdown");
    }

    byId("btnMonLaunch").disabled = isActive;
    byId("btnMonRestart").disabled = !(phase === "running" || phase === "countdown");
    byId("btnMonStop").disabled = isStopped || phase === "stopping";
    byId("btnLaunchFromSetup").disabled = isActive;

    byId("teleStatus").textContent = phaseLabel(phase);
    byId("telePid").textContent = "PID: " + (st.pid ? String(st.pid) : "—");

    const portModeText = st.switchPortOnRestart
      ? "切替: " + st.serverPorts[0] + " ↔ " + st.serverPorts[1]
      : "固定: " + st.serverPorts[0];
    byId("telePort").textContent = st.activePort ? String(st.activePort) : "—";
    byId("teleUdp").textContent = st.udpConfirmed
      ? "UDPリッスン確認済 (" + portModeText + ")"
      : portModeText;

    byId("teleMap").textContent = st.mapLabel || "—";
    byId("teleRound").textContent = "ラウンド: #" + (st.round || 0) + (st.mapId ? " (" + st.mapId + ")" : "");

    const totalPatches = st.totalPatchCount || 0;
    const injected = st.injectedCount || 0;
    byId("teleMods").textContent = injected + " / " + totalPatches;
    byId("teleWatchdog").textContent = "自動再起動ウォッチドッグ: " + (state.autoRestartWatchdog ? "ON" : "OFF");

    byId("monLogFileBadge").textContent = st.currentPatchLogFile
      ? "パッチログ: " + st.currentPatchLogFile
      : "パッチログ: 未生成";

    const inlineCountdown = byId("countdownInline");
    const portChip = byId("countdownPort");
    if (isCountdown) {
      inlineCountdown.classList.add("is-visible");
      byId("countdownHeadline").textContent = "自動再起動 " + st.countdownSeconds + "秒";
      if (st.nextPort) {
        portChip.textContent = "Port " + st.nextPort;
        portChip.style.display = "inline-flex";
      } else {
        portChip.style.display = "none";
      }
    } else {
      inlineCountdown.classList.remove("is-visible");
    }

    byId("injectProgressChip").textContent = injected + " / " + totalPatches;
    const pct = totalPatches > 0 ? Math.min(100, Math.round((injected / totalPatches) * 100)) : 0;
    byId("injectProgressBar").style.width = pct + "%";

    const listEl = byId("patchProgressList");
    listEl.textContent = "";
    if (!st.patchProgress || st.patchProgress.length === 0) {
      const empty = document.createElement("div");
      empty.className = "log-empty";
      empty.textContent = "サーバーを起動すると、Frida Gadget 注入進捗がリアルタイムに表示されます。";
      listEl.appendChild(empty);
    } else {
      st.patchProgress.forEach(function (item) {
        const row = document.createElement("div");
        row.className =
          "patch-item" +
          (item.status === "loaded" ? " is-loaded" : item.status === "error" ? " is-error" : "");

        const left = document.createElement("div");
        left.className = "patch-item-left";

        const idxChip = document.createElement("span");
        idxChip.className =
          item.category === "client_required"
            ? "chip purple"
            : item.category === "default"
            ? "chip dark"
            : "chip info";
        idxChip.textContent = "#" + item.index;

        const nameEl = document.createElement("span");
        nameEl.className = "patch-item-name";
        nameEl.textContent = item.label + " (" + item.fileName + ")";

        left.appendChild(idxChip);
        left.appendChild(nameEl);

        const stChip = document.createElement("span");
        if (item.status === "loaded") {
          stChip.className = "chip ok";
          stChip.textContent = "注入完了";
        } else if (item.status === "error") {
          stChip.className = "chip err";
          stChip.textContent = "エラー";
        } else {
          stChip.className = "chip";
          stChip.textContent = "待機中";
        }

        row.appendChild(left);
        row.appendChild(stChip);
        listEl.appendChild(row);
      });
    }
  }

  // ==================== Tab 04: Basic Settings ====================
  function renderConfigForm() {
    const cfg = state.configDraft || state.config;
    if (!cfg) return;
    byId("cfgShippingExe").value = cfg.shippingExecutable || "";
    byId("cfgPatchPort").value = String(cfg.consolePatchPort || 7893);
    byId("cfgPort1").value = String(cfg.serverPorts ? cfg.serverPorts[0] : 7895);
    byId("cfgPort2").value = String(cfg.serverPorts ? cfg.serverPorts[1] : 7897);
    byId("cfgRestartWait").value = String(cfg.restartWaitSeconds || 10);
    byId("cfgSwitchPort").checked = Boolean(cfg.switchPortOnRestart);
    byId("cfgAutoBroadcast").checked = Boolean(cfg.autoBroadcastDefault);
    byId("cfgWatchdog").checked = Boolean(state.autoRestartWatchdog);

    byId("cfgPort2").disabled = !cfg.switchPortOnRestart;
    renderBridgeLabels();
    renderExeStatus();
  }

  function renderBridgeLabels() {
    const activePort = state.config ? state.config.consolePatchPort : 7893;
    byId("hdrBridgeText").textContent = "内部連携: 127.0.0.1:" + activePort;
    byId("conLoopbackPortLabel").textContent = "127.0.0.1:" + activePort;
    byId("tabConsoleBadge").textContent = ":" + activePort;
  }

  function renderExeStatus() {
    const exeChip = byId("exeExistsChip");
    const legacyChip = byId("legacyFridaChip");
    const alertStrip = byId("globalAlertStrip");
    const alertText = byId("globalAlertText");

    if (state.serverExeExists) {
      exeChip.textContent = "実行ファイル検出済 (OK)";
      exeChip.className = "chip ok";
    } else {
      exeChip.textContent = "未検出 — パスを確認してください";
      exeChip.className = "chip err";
    }

    legacyChip.style.display = state.legacyFridaDllDetected ? "inline-flex" : "none";

    if (!state.serverExeExists) {
      alertStrip.className = "alert-strip is-visible is-danger";
      alertText.textContent =
        "サーバー実行ファイルが見つかりません (" +
        (state.config ? state.config.shippingExecutable : "") +
        ") — 「基本設定」タブで実行ファイルを選択またはサーバーフォルダへ配置してください。";
    } else if (state.legacyFridaDllDetected) {
      alertStrip.className = "alert-strip is-visible";
      alertText.textContent =
        "警告: 旧パッチ用の frida.dll がサーバーディレクトリ内に検出されました。未改造サーバー本体での安定稼働のため削除を推奨します。";
    } else {
      alertStrip.className = "alert-strip";
    }
  }

  function buildConfigFromForm() {
    const base = state.config || {};
    const shippingExe = byId("cfgShippingExe").value.trim();
    const port1 = parseInt(byId("cfgPort1").value, 10) || 7895;
    const port2 = parseInt(byId("cfgPort2").value, 10) || 7897;
    const switchPort = Boolean(byId("cfgSwitchPort").checked);
    return {
      serverPorts: [port1, switchPort ? port2 : port1 === 7897 ? 7895 : 7897],
      switchPortOnRestart: switchPort,
      consolePatchPort: parseInt(byId("cfgPatchPort").value, 10) || 7893,
      restartWaitSeconds: parseInt(byId("cfgRestartWait").value, 10) || 10,
      autoBroadcastDefault: Boolean(byId("cfgAutoBroadcast").checked),
      serverExecutable: base.serverExecutable || "./DreadHungerServer.exe",
      shippingExecutable: shippingExe,
      shippingProcessName:
        extractFileName(shippingExe) ||
        base.shippingProcessName ||
        "DreadHungerServer-Win64-Shipping.exe",
    };
  }

  function buildLaunchRequest() {
    return {
      mapId: state.selectedMapId,
      settings: readMatchSettingsFromForm(),
      selectedContentNumbers: Array.from(state.selectedContentNumbers).sort(function (a, b) { return a - b; }),
      selectedOptionNumbers: Array.from(state.selectedOptionNumbers).sort(function (a, b) { return a - b; }),
      autoRestartWatchdog: Boolean(byId("cfgWatchdog").checked),
    };
  }

  function renderLogEntries(containerId, entries, emptyText, isConsole) {
    const box = byId(containerId);
    if (!box) return;
    box.textContent = "";
    if (!entries || entries.length === 0) {
      const empty = document.createElement("div");
      empty.className = "log-empty";
      empty.textContent = emptyText;
      box.appendChild(empty);
      return;
    }
    const frag = document.createDocumentFragment();
    entries.forEach(function (entry) {
      const line = document.createElement("div");
      line.className = "log-line";

      const timeEl = document.createElement("span");
      timeEl.className = "log-time";
      timeEl.textContent = "[" + entry.time + "]";

      const tagEl = document.createElement("span");
      const tone = isConsole ? (entry.tone || "info") : (entry.level || "info");
      tagEl.className = "log-tag tone-" + tone;
      tagEl.textContent = isConsole ? entry.category : (entry.level || "INFO").toUpperCase();

      const msgEl = document.createElement("span");
      msgEl.className = "log-msg selectable";
      msgEl.textContent = isConsole ? entry.detail : entry.message;

      line.appendChild(timeEl);
      line.appendChild(tagEl);
      line.appendChild(msgEl);
      frag.appendChild(line);
    });
    box.appendChild(frag);
    box.scrollTop = box.scrollHeight;
  }

  // ==================== Event Bindings ====================
  function bindEvents() {
    // Custom frameless window controls
    byId("btnWinMin").addEventListener("click", function () {
      callBackend("window_minimize").catch(function () {});
    });

    byId("btnWinClose").addEventListener("click", function () {
      callBackend("window_close").catch(function () {});
    });

    // Tabs
    document.querySelectorAll(".nav-tab").forEach(function (btn) {
      btn.addEventListener("click", function () {
        switchTab(btn.getAttribute("data-tab"), false);
      });
    });

    byId("btnAlertGotoSettings").addEventListener("click", function () {
      switchTab("settings", false);
      byId("cfgShippingExe").focus();
    });

    // Console operation tiles
    document.querySelectorAll("[data-op]").forEach(function (btn) {
      btn.addEventListener("click", function () {
        if (!state.consoleDraft) return;
        state.consoleDraft.operation = btn.getAttribute("data-op");
        renderConsoleVisibility();
        scheduleDraftSync(true);
      });
    });

    byId("btnTargetRole").addEventListener("click", function () {
      if (!state.consoleDraft) return;
      state.consoleDraft.targetMode = "role";
      renderConsoleVisibility();
      scheduleDraftSync(true);
    });

    byId("btnTargetCustom").addEventListener("click", function () {
      if (!state.consoleDraft) return;
      state.consoleDraft.targetMode = "custom";
      renderConsoleVisibility();
      scheduleDraftSync(true);
    });

    // Console dynamic form changes -> immediate sync
    [
      "conTargetRole",
      "conCommandType",
      "conSummon",
      "conIsThrall",
      "conSpellCharge",
      "conBlizzard",
      "conSlomo",
      "conGod",
      "conEndMatch",
      "conStatusProp",
      "conStatusToggle",
      "conItemSelect",
      "conChatBroadcast",
      "conAutoBroadcast",
    ].forEach(function (id) {
      byId(id).addEventListener("change", function () {
        renderConsoleVisibility();
        scheduleDraftSync(true);
      });
    });

    [
      "conCustomTarget",
      "conRawCommand",
      "conStatusNumber",
      "conChatMessage",
    ].forEach(function (id) {
      byId(id).addEventListener("input", function () {
        scheduleDraftSync(false);
      });
    });

    byId("conItemCount").addEventListener("input", function () {
      renderItemCountPresets();
      scheduleDraftSync(false);
    });

    byId("conStorageLimit").addEventListener("input", function () {
      renderStoragePresets();
      scheduleDraftSync(false);
    });

    // Console submit
    byId("btnConsoleSubmit").addEventListener("click", async function () {
      if (state.submittingConsole) return;
      const req = buildConsoleActionRequest();
      if (!req) return;
      const btn = byId("btnConsoleSubmit");
      const face = btn.querySelector(".nb-cta-face") || btn;
      state.submittingConsole = true;
      btn.disabled = true;
      face.textContent = "送信中...";
      try {
        const res = await callBackend("execute_admin_console_action", { request: req });
        const toastType = res.ok
          ? (res.code === "API_HIGH_VALUE_WARNING" ? "warn" : "success")
          : "error";
        showToast(res.message, toastType);
      } catch (err) {
        showToast(String(err), "error");
      } finally {
        state.submittingConsole = false;
        btn.disabled = false;
        face.textContent = "コンソールへ送信";
      }
    });

    byId("btnClearConsoleLogs").addEventListener("click", async function () {
      try {
        await callBackend("clear_console_logs");
        state.consoleLogs = [];
        renderLogEntries(
          "consoleLogBox",
          state.consoleLogs,
          "コンソールから送信した操作ログがここにリアルタイム表示されます。",
          true
        );
      } catch (err) {
        showToast(String(err), "error");
      }
    });

    // Settings form inputs -> live draft sync
    [
      "cfgShippingExe",
      "cfgPort1",
      "cfgPort2",
      "cfgPatchPort",
      "cfgRestartWait",
    ].forEach(function (id) {
      byId(id).addEventListener("input", function () {
        scheduleDraftSync(false);
      });
    });

    byId("cfgSwitchPort").addEventListener("change", function () {
      byId("cfgPort2").disabled = !byId("cfgSwitchPort").checked;
      scheduleDraftSync(true);
    });

    byId("cfgAutoBroadcast").addEventListener("change", function () {
      scheduleDraftSync(true);
    });

    byId("cfgWatchdog").addEventListener("change", function () {
      state.autoRestartWatchdog = Boolean(byId("cfgWatchdog").checked);
      renderServerMonitor();
      scheduleDraftSync(true);
    });

    byId("btnPickExe").addEventListener("click", async function () {
      try {
        const picked = await callBackend("pick_server_executable");
        if (picked && picked.path) {
          byId("cfgShippingExe").value = picked.path;
          state.serverExeExists = Boolean(picked.exists);
          renderExeStatus();
          scheduleDraftSync(true);
          showToast("実行ファイルを選択しました。「./launcher_config.json に保存」で反映できます。", "success");
        }
      } catch (err) {
        showToast(String(err), "error");
      }
    });

    byId("btnSaveConfig").addEventListener("click", async function () {
      try {
        const cfg = buildConfigFromForm();
        const watchdog = Boolean(byId("cfgWatchdog").checked);
        const res = await callBackend("save_config", {
          config: cfg,
          autoRestartWatchdog: watchdog,
        });
        state.config = res.config;
        state.configDraft = res.config;
        state.autoRestartWatchdog = res.autoRestartWatchdog;
        state.serverExeExists = res.serverExeExists;
        state.legacyFridaDllDetected = res.legacyFridaDllDetected;
        byId("conAutoBroadcast").checked = Boolean(res.config.autoBroadcastDefault);
        renderConfigForm();
        renderServerMonitor();
        scheduleDraftSync(true);
        showToast("./launcher_config.json に設定を保存しました！", "success");
      } catch (err) {
        showToast(String(err), "error");
      }
    });

    byId("btnResetConfig").addEventListener("click", async function () {
      try {
        const defCfg = await callBackend("reset_default_config");
        state.configDraft = defCfg;
        state.autoRestartWatchdog = true;
        renderConfigForm();
        scheduleDraftSync(true);
        showToast("フォームをデフォルト値に戻しました（保存ボタンでファイル反映）", "warn");
      } catch (err) {
        showToast(String(err), "error");
      }
    });

    // Match rules
    [
      "ruleMaxPlayers",
      "ruleDaysBlizzard",
      "ruleDayMinutes",
      "ruleThralls",
      "rulePredatorDmg",
      "ruleCold",
      "ruleHunger",
      "ruleCoal",
    ].forEach(function (id) {
      byId(id).addEventListener("input", function () {
        renderUeArgPreview();
        scheduleDraftSync(false);
        scheduleNextRoundAutoSync();
      });
    });

    byId("btnResetRules").addEventListener("click", function () {
      applyMatchSettingsToForm({
        maxPlayers: 8,
        daysBeforeBlizzard: 3,
        dayMinutes: 9,
        predatorDamage: 1.0,
        coldIntensity: 1.0,
        hungerRate: 1.0,
        coalBurnRate: 1.0,
        thralls: 2,
      });
      scheduleDraftSync(true);
      scheduleNextRoundAutoSync();
      showToast("試合ルールをデフォルトに戻しました。", "success");
    });

    byId("btnClearMods").addEventListener("click", function () {
      state.selectedContentNumbers.clear();
      state.selectedOptionNumbers.clear();
      renderModGrids();
      scheduleDraftSync(true);
      scheduleNextRoundAutoSync();
      showToast("選択中のMODをすべてクリアしました。", "warn");
    });

    // Server Launch / Restart / Stop / Countdown
    async function triggerLaunch() {
      try {
        const req = buildLaunchRequest();
        const st = await callBackend("launch_server", { request: req });
        state.serverStatus = st;
        renderServerMonitor();
        switchTab("monitor", false);
        showToast("サーバー起動・Frida注入シーケンスを開始しました！", "success");
      } catch (err) {
        showToast(String(err), "error");
      }
    }

    byId("btnLaunchFromSetup").addEventListener("click", triggerLaunch);
    byId("btnMonLaunch").addEventListener("click", triggerLaunch);

    async function triggerRestart() {
      try {
        await callBackend("restart_server_now");
        showToast("サーバーの即時再起動を要求しました。", "warn");
      } catch (err) {
        showToast(String(err), "error");
      }
    }

    async function triggerStop() {
      try {
        await callBackend("stop_server");
        showToast("サーバー停止処理を実行しています。", "warn");
      } catch (err) {
        showToast(String(err), "error");
      }
    }

    byId("btnMonRestart").addEventListener("click", triggerRestart);
    byId("btnMonStop").addEventListener("click", triggerStop);

    byId("btnCountdownReconfig").addEventListener("click", async function () {
      try {
        await callBackend("cancel_countdown_for_reconfigure");
        switchTab("mods", false);
        showToast("自動再起動を停止しました。マップ・MODを変更して起動してください。", "success");
      } catch (err) {
        showToast(String(err), "error");
      }
    });

    byId("btnClearServerLogs").addEventListener("click", async function () {
      try {
        await callBackend("clear_server_logs");
        state.serverLogs = [];
        renderLogEntries(
          "serverLogBox",
          state.serverLogs,
          "サーバー起動・ウォッチドッグ・再起動イベントがここに表示されます。",
          false
        );
      } catch (err) {
        showToast(String(err), "error");
      }
    });

    byId("btnOpenLogsDir").addEventListener("click", async function () {
      try {
        await callBackend("open_logs_folder");
      } catch (err) {
        showToast(String(err), "error");
      }
    });

    // In-App License Modal
    const licenseBackdrop = byId("licenseModalBackdrop");
    const licenseTextEl = byId("licenseModalText");
    const closeLicenseBtn = byId("btnCloseLicenseModal");

    function closeLicenseModal() {
      if (!licenseBackdrop) return;
      licenseBackdrop.classList.remove("is-open");
      licenseBackdrop.setAttribute("aria-hidden", "true");
    }

    async function openLicenseModal() {
      if (!licenseBackdrop || !licenseTextEl) return;
      licenseBackdrop.classList.add("is-open");
      licenseBackdrop.setAttribute("aria-hidden", "false");
      if (state.licenseText === null) {
        licenseTextEl.textContent = "読み込み中...";
        try {
          state.licenseText = await callBackend("get_license_text");
        } catch (err) {
          licenseTextEl.textContent = "ライセンス情報の読み込みに失敗しました: " + String(err);
          return;
        }
      }
      licenseTextEl.textContent = state.licenseText;
      licenseTextEl.scrollTop = 0;
      if (closeLicenseBtn) closeLicenseBtn.focus();
    }

    byId("btnShowLicense").addEventListener("click", openLicenseModal);
    closeLicenseBtn.addEventListener("click", closeLicenseModal);
    licenseBackdrop.addEventListener("click", function (event) {
      if (event.target === licenseBackdrop) {
        closeLicenseModal();
      }
    });
    document.addEventListener("keydown", function (event) {
      if (event.key === "Escape" && licenseBackdrop.classList.contains("is-open")) {
        event.preventDefault();
        closeLicenseModal();
      }
    });
  }

  async function subscribeTauriEvents() {
    if (!listen) return;
    await listen("dh://console-log", function (ev) {
      if (!ev || !ev.payload) return;
      state.consoleLogs.push(ev.payload);
      if (state.consoleLogs.length > 300) state.consoleLogs.shift();
      renderLogEntries(
        "consoleLogBox",
        state.consoleLogs,
        "コンソールから送信した操作ログがここにリアルタイム表示されます。",
        true
      );
    });

    await listen("dh://server-log", function (ev) {
      if (!ev || !ev.payload) return;
      state.serverLogs.push(ev.payload);
      if (state.serverLogs.length > 400) state.serverLogs.shift();
      renderLogEntries(
        "serverLogBox",
        state.serverLogs,
        "サーバー起動・ウォッチドッグ・再起動イベントがここに表示されます。",
        false
      );
    });

    await listen("dh://server-state", function (ev) {
      if (!ev || !ev.payload) return;
      state.serverStatus = ev.payload;
      renderServerMonitor();
    });
  }

  async function init() {
    nbSelect.enhanceAll();
    bindEvents();
    await subscribeTauriEvents();
    try {
      const snap = await callBackend("get_initial_state");
      state.config = snap.config;
      state.serverExeExists = Boolean(snap.serverExeExists);
      state.legacyFridaDllDetected = Boolean(snap.legacyFridaDllDetected);
      state.consoleCatalog = snap.consoleCatalog;
      state.maps = snap.maps || [];
      state.defaultMods = snap.defaultMods || [];
      state.clientRequiredMods = snap.clientRequiredMods || [];
      state.practicalMods = snap.practicalMods || [];
      state.funMods = snap.funMods || [];
      state.conflictPairs = snap.conflictPairs || [];
      state.serverStatus = snap.serverStatus;
      state.consoleLogs = snap.consoleLogs || [];
      state.serverLogs = snap.serverLogs || [];

      if (snap.appVersion) {
        byId("footerVersion").textContent = "DH_ServerLauncher_JA v" + snap.appVersion;
      }

      const draft = snap.uiDraft;
      if (draft) {
        state.activeTab = draft.activeTab || "console";
        state.consoleDraft = draft.consoleDraft;
        state.configDraft = draft.configDraft || snap.config;
        state.autoRestartWatchdog = Boolean(draft.autoRestartWatchdog);
        if (draft.matchDraft) {
          state.selectedMapId = draft.matchDraft.mapId || "Departure_Persistent";
          applyMatchSettingsToForm(draft.matchDraft.settings);
          state.selectedContentNumbers = new Set(draft.matchDraft.selectedContentNumbers || []);
          state.selectedOptionNumbers = new Set(draft.matchDraft.selectedOptionNumbers || []);
        } else {
          applyMatchSettingsToForm(snap.defaultMatchSettings);
        }
      } else {
        state.configDraft = snap.config;
        state.autoRestartWatchdog = Boolean(snap.autoRestartWatchdog);
        applyMatchSettingsToForm(snap.defaultMatchSettings);
      }

      hydrateConsoleCatalogAndDraft();
      renderConfigForm();
      renderMaps();
      renderDefaultMods();
      renderModGrids();
      renderServerMonitor();
      renderLogEntries(
        "consoleLogBox",
        state.consoleLogs,
        "コンソールから送信した操作ログがここにリアルタイム表示されます。",
        true
      );
      renderLogEntries(
        "serverLogBox",
        state.serverLogs,
        "サーバー起動・ウォッチドッグ・再起動イベントがここに表示されます。",
        false
      );
      switchTab(state.activeTab, true);

      if (snap.configNotice) {
        showToast(snap.configNotice, snap.configOutcome === "load_failed" ? "warn" : "success");
      }
    } catch (err) {
      showToast("初期状態の読み込みに失敗しました: " + String(err), "error");
    }
  }

  document.addEventListener("DOMContentLoaded", init);
})();
