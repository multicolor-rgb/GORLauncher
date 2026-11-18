// goglite — frontend (invoke, listen, render) + wielojęzyczność (PL/EN)
// Aplikacja ogranicza się do przeglądania biblioteki GOG i pobierania instalatorów offline.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t, setLanguage, currentLang } from "./i18n.js";

/** Wykrywa system operacyjny (Windows/Linux/macOS) — do domyślnego filtra platformy. */
const detectOs = () => invoke("detect_os");

const $ = (s) => document.querySelector(s);
const show = (e) => e.classList.remove("hidden");
const hide = (e) => e.classList.add("hidden");

const d = {
  // sidebar / nawigacja
  navGames: $("#nav-games"), navDls: $("#nav-downloads"), navSettings: $("#nav-settings"),
  // topbar
  searchInput: $("#search-input"), topSettingsBtn: $("#btn-settings"),
  // auth
  loginBtn: $("#btn-login"), loginError: $("#login-error"), welcome: $("#welcome"),
  logoutBtn: $("#btn-logout"), syncBtn: $("#btn-sync"), statusBadge: $("#status-badge"),
  // biblioteka
  gameSection: $("#game-section"), gameGrid: $("#game-grid"),
  gameEmpty: $("#game-empty"), gameCount: $("#game-count"),
  platformFilter: $("#platform-filter"),
  syncBar: $("#sync-progress"), syncProg: $("#sync-progress-bar"),
  // pobierania
  dlList: $("#download-list"), dlBadge: $("#dl-badge"),
  dlDirLabel: $("#dl-dir-label"), dlSettingsBtn: $("#btn-dl-settings"),
  // ustawienia
  setDirLabel: $("#set-dir-label"), chooseDirBtn: $("#btn-choose-dir"), langSwitch: $("#lang-switch"),
  themeSwitch: $("#theme-switch"),
  // widoki
  viewGames: $("#view-games"), viewDls: $("#view-downloads"), viewSettings: $("#view-settings"),
  // modal
  modal: $("#game-modal"), modalTitle: $("#modal-title"), modalHeroImg: $("#modal-hero-img"),
  modalBody: $("#modal-body"), closeModalBtn: $("#btn-close-modal"), modalTabs: $("#modal-tabs"),
  screenshotViewer: $("#screenshot-viewer"), screenshotViewerImg: $("#screenshot-viewer-img"),
  screenshotViewerClose: $("#screenshot-viewer-close"),
  // Amazon Games (Prime Gaming)
  navAmazon: $("#nav-amazon"), viewAmazon: $("#view-amazon"),
  amazonSetup: $("#amazon-setup"), amazonSetupMsg: $("#amazon-setup-msg"),
  amazonCopyBtn: $("#btn-amazon-copy"), amazonRetryBtn: $("#btn-amazon-retry"),
  amazonInstallNileBtn: $("#btn-amazon-install-nile"), amazonSetupError: $("#amazon-setup-error"),
  amazonSetupProgress: $("#amazon-setup-progress"),
  amazonBar: $("#amazon-bar"), amazonUser: $("#amazon-user"),
  amazonLoginBtn: $("#btn-amazon-login"), amazonLogoutBtn: $("#btn-amazon-logout"),
  amazonRefreshBtn: $("#btn-amazon-refresh"),
  amazonLoginPanel: $("#amazon-login-panel"), amazonLoginUrl: $("#amazon-login-url"),
  amazonLoginFinishBtn: $("#btn-amazon-login-finish"), amazonLoginCancelBtn: $("#btn-amazon-login-cancel"),
  amazonLoginError: $("#amazon-login-error"),
  amazonEmpty: $("#amazon-empty"), amazonGrid: $("#amazon-grid"),
  // Pobrania sklepów (Prime Gaming + Epic) pokazujemy w zakładce Pobierania, nad pobraniami GOG.
  dlStores: $("#dl-stores"),
  // Epic Games
  navEpic: $("#nav-epic"), viewEpic: $("#view-epic"),
  epicSetup: $("#epic-setup"), epicSetupMsg: $("#epic-setup-msg"),
  epicCopyBtn: $("#btn-epic-copy"), epicRetryBtn: $("#btn-epic-retry"),
  epicInstallToolBtn: $("#btn-epic-install-tool"), epicSetupError: $("#epic-setup-error"),
  epicSetupProgress: $("#epic-setup-progress"),
  epicBar: $("#epic-bar"), epicUser: $("#epic-user"),
  epicLoginBtn: $("#btn-epic-login"), epicLogoutBtn: $("#btn-epic-logout"),
  epicRefreshBtn: $("#btn-epic-refresh"),
  epicLoginPanel: $("#epic-login-panel"), epicLoginCode: $("#epic-login-code"),
  epicLoginFinishBtn: $("#btn-epic-login-finish"), epicLoginCancelBtn: $("#btn-epic-login-cancel"),
  epicLoginError: $("#epic-login-error"),
  epicEmpty: $("#epic-empty"), epicGrid: $("#epic-grid"),
};

let loggedIn = false;
/** id pobierania -> pełny wiersz (scalamy z cząstkowymi zdarzeniami postępu). */
const dlRows = new Map();
/** Aktualnie ustawiony folder zapisu (pusty = trzeba go wybrać przed pobraniem). */
let currentDir = "";
/** Pełna lista gier z bazy (do filtrowania wyszukiwarką i platformą). */
let gamesCache = [];
/** Zapytanie wyszukiwarki (małe litery). */
let searchQuery = "";
/** Bieżący motyw interfejsu: "dark" lub "light". */
let currentTheme = "dark";
/** Aktywna zakładka modala: "info" (opis + zrzuty) lub "installers" (instalatory). */
let activeModalTab = "info";
/** System operacyjny z backendu: "linux" | "windows" | "macos". */
let currentOs = "linux";
/** Filtr platformy w bibliotece: "all" | "windows" | "linux" | "macos". */
let platformFilter = "all";
/** Filtr platformy na liście instalatorów w modalu: "all" | "windows" | "linux" | "macos". */
let installerFilter = "all";
/** Instalatory aktualnie otwartej gry (do filtrowania bez ponownego zapytania). */
let modalInstallers = [];
/** Id gry aktualnie otwartej w modalu. */
let currentGameId = null;

const VIEWS = {
  games: { view: d.viewGames, nav: d.navGames },
  amazon: { view: d.viewAmazon, nav: d.navAmazon },
  epic: { view: d.viewEpic, nav: d.navEpic },
  downloads: { view: d.viewDls, nav: d.navDls },
  settings: { view: d.viewSettings, nav: d.navSettings },
};

/** Przełącza aktywny widok i podświetla odpowiednią pozycję nawigacji. */
function switchView(v) {
  Object.values(VIEWS).forEach((x) => { x.view.classList.remove("active"); x.nav.classList.remove("active"); });
  const target = VIEWS[v] || VIEWS.games;
  target.view.classList.add("active");
  target.nav.classList.add("active");
}

/** Etykieta statusu pobierania w bieżącym języku. */
function statusLabel(s) { return t("status." + s, s); }

async function init() {
  // Nawigacja
  // Przełączenie widoku odświeża stany plików (sprawdzenie, czy instalatory nadal są na dysku).
  d.navGames.addEventListener("click", () => { switchView("games"); loadDownloads(); });
  d.navDls.addEventListener("click", () => { switchView("downloads"); loadDownloads(); });
  d.navAmazon.addEventListener("click", () => { switchView("amazon"); loadAmazon(); });
  d.navEpic.addEventListener("click", () => { switchView("epic"); loadEpic(); });
  d.navSettings.addEventListener("click", () => switchView("settings"));
  d.topSettingsBtn.addEventListener("click", () => switchView("settings"));
  d.dlSettingsBtn.addEventListener("click", () => switchView("settings"));
  // Akcje
  d.loginBtn.addEventListener("click", doSetToken);
  d.logoutBtn.addEventListener("click", doLogout);
  d.syncBtn.addEventListener("click", sync);
  d.closeModalBtn.addEventListener("click", closeModal);
  d.modal.addEventListener("click", (e) => { if (e.target === d.modal) closeModal(); });
  // Zakładki modala (Opis / Instalatory)
  d.modalTabs.addEventListener("click", (e) => {
    const btn = e.target.closest(".modal-tab");
    if (btn) switchModalTab(btn.getAttribute("data-tab"));
  });
  // Podgląd zrzutów ekranu (lightbox wewnątrz modala)
  d.screenshotViewerClose.addEventListener("click", closeScreenshotViewer);
  d.screenshotViewer.addEventListener("click", closeScreenshotViewer);
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && !d.screenshotViewer.classList.contains("hidden")) {
      e.preventDefault();
      closeScreenshotViewer();
    }
  });
  d.chooseDirBtn.addEventListener("click", () => chooseDownloadDir());
  // Amazon Games: logowanie, odświeżanie biblioteki i akcje na kafelkach
  d.amazonLoginBtn.addEventListener("click", amazonLogin);
  d.amazonLogoutBtn.addEventListener("click", amazonLogout);
  d.amazonRefreshBtn.addEventListener("click", () => loadAmazon(true));
  d.amazonRetryBtn.addEventListener("click", () => loadAmazon());
  d.amazonCopyBtn.addEventListener("click", amazonCopyCommand);
  d.amazonInstallNileBtn.addEventListener("click", amazonInstallNile);
  d.amazonLoginFinishBtn.addEventListener("click", amazonLoginFinish);
  d.amazonLoginCancelBtn.addEventListener("click", amazonLoginCancel);
  d.amazonLoginUrl.addEventListener("keydown", (e) => { if (e.key === "Enter") amazonLoginFinish(); });
  d.amazonGrid.addEventListener("click", onAmazonGridClick);
  // Epic Games: logowanie, odświeżanie biblioteki i akcje na kafelkach
  d.epicLoginBtn.addEventListener("click", epicLogin);
  d.epicLogoutBtn.addEventListener("click", epicLogout);
  d.epicRefreshBtn.addEventListener("click", () => loadEpic(true));
  d.epicRetryBtn.addEventListener("click", () => loadEpic());
  d.epicCopyBtn.addEventListener("click", epicCopyCommand);
  d.epicInstallToolBtn.addEventListener("click", epicInstallTool);
  d.epicLoginFinishBtn.addEventListener("click", epicLoginFinish);
  d.epicLoginCancelBtn.addEventListener("click", epicLoginCancel);
  d.epicLoginCode.addEventListener("keydown", (e) => { if (e.key === "Enter") epicLoginFinish(); });
  d.epicGrid.addEventListener("click", onEpicGridClick);
  // Wyszukiwarka
  d.searchInput.addEventListener("input", () => {
    searchQuery = d.searchInput.value.trim().toLowerCase();
    renderGames();
  });
  // Przełącznik języka
  d.langSwitch.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-lang]");
    if (btn) applyLanguage(btn.getAttribute("data-lang"), true);
  });
  // Przełącznik motywu (jasny / ciemny)
  d.themeSwitch.addEventListener("click", (e) => {
    const btn = e.target.closest("[data-theme-opt]");
    if (btn) applyTheme(btn.getAttribute("data-theme-opt"), true);
  });
  // Filtr platformy w bibliotece (Wszystkie / Windows / Linux / macOS)
  if (d.platformFilter) {
    d.platformFilter.addEventListener("click", (e) => {
      const btn = e.target.closest("[data-platform]");
      if (btn) selectPlatformFilter(btn.getAttribute("data-platform"));
    });
  }

  // Język: zapisane ustawienie > język systemu (fallback pl).
  let savedLang = null, savedTheme = null;
  try { const st = await invoke("get_settings"); savedLang = st && st.language; savedTheme = st && st.theme; } catch (e) {}
  applyLanguage(savedLang || navigator.language, false);
  // Motyw: zapisane ustawienie > domyślny ciemny.
  applyTheme(savedTheme || "dark", false);

  // Wykrycie systemu — domyślnie zaznaczamy platformę bieżącego systemu w filtrze.
  try { const info = await detectOs(); if (info && info.os) currentOs = info.os; } catch (e) {}
  selectPlatformFilter(currentOs);

  try { const st = await invoke("get_auth_status"); loggedIn = st.loggedIn; } catch (e) {}
  renderAuth();
  await refreshDownloadDir();
  await loadDownloads();
  if (loggedIn) { await loadGames(); show(d.syncBtn); }

  listen("auth:changed", (e) => {
    loggedIn = e.payload.loggedIn; renderAuth();
    if (loggedIn) { hide(d.welcome); show(d.syncBtn); sync(); }
    else { gamesCache = []; show(d.welcome); hide(d.gameSection); hide(d.gameEmpty); hide(d.syncBtn); }
  });
  listen("auth:error", (e) => {
    const msg = (e.payload && e.payload.message) || t("auth.unknown");
    d.loginError.textContent = t("auth.loginError") + ": " + msg;
    show(d.loginError);
    d.loginBtn.disabled = false; d.loginBtn.textContent = t("welcome.login");
  });
  listen("library:page", (e) => { show(d.syncBar); d.syncProg.value = e.payload.progress || 0; });
  listen("download:progress", (e) => {
    const row = dlRows.get(e.payload.id);
    if (!row) return;
    Object.assign(row, e.payload);
    const el = document.querySelector(`[data-dl-id="${e.payload.id}"]`);
    if (!el) return;
    // Postęp zmienia tylko pasek/liczby — nie przebudowujemy przycisków (stabilne klikanie).
    if (el.dataset.status === row.status) patchDlProgress(el, row);
    else paintDl(el, row);
  });
  listen("download:state", () => loadDownloads());
  listen("amazon:progress", (e) => {
    const a = amazonActive.get(e.payload.gameId);
    if (!a) return;
    if (e.payload.line) a.line = e.payload.line;
    if (e.payload.percent != null) a.percent = e.payload.percent;
    renderStoreDownloads();
  });
  listen("amazon:done", (e) => {
    const { gameId, ok, error } = e.payload;
    const a = amazonActive.get(gameId);
    if (a) {
      a.ok = !!ok; a.error = error || "";
      if (ok) a.percent = 100;
    }
    renderStoreDownloads();
    loadAmazon(false);
  });
  listen("amazon:nile-progress", (e) => {
    const line = e.payload && e.payload.line;
    if (!line || !d.amazonSetupProgress) return;
    d.amazonSetupProgress.textContent = line;
    show(d.amazonSetupProgress);
  });
  listen("epic:progress", (e) => {
    const a = epicActive.get(e.payload.gameId);
    if (!a) return;
    if (e.payload.line) a.line = e.payload.line;
    if (e.payload.percent != null) a.percent = e.payload.percent;
    renderStoreDownloads();
  });
  listen("epic:done", (e) => {
    const { gameId, ok, error } = e.payload;
    const a = epicActive.get(gameId);
    if (a) {
      a.ok = !!ok; a.error = error || "";
      if (ok) a.percent = 100;
    }
    renderStoreDownloads();
    loadEpic(false);
  });
  listen("epic:tool-progress", (e) => {
    const line = e.payload && e.payload.line;
    if (!line || !d.epicSetupProgress) return;
    d.epicSetupProgress.textContent = line;
    show(d.epicSetupProgress);
  });
}

/** Ustawia język, tłumaczy cały interfejs i (opcjonalnie) zapisuje wybór. */
function applyLanguage(lang, persist) {
  const norm = setLanguage(lang);
  renderAuth();
  if (loggedIn) renderGames();
  renderDownloads();
  renderAmazonUi();
  renderEpicUi();
  setDirLabel(currentDir);
  if (d.modal.open) {
    const loadingEl = d.modalBody.querySelector(".loading");
    if (loadingEl) loadingEl.textContent = t("modal.loading");
  }
  if (persist) { invoke("set_language", { language: norm }).catch(() => {}); }
}

/** Ustawia motyw (`"light"`/`"dark"`), podświetla przełącznik i (opcjonalnie) zapisuje wybór. */
function applyTheme(theme, persist) {
  const norm = theme === "light" ? "light" : "dark";
  currentTheme = norm;
  document.documentElement.setAttribute("data-theme", norm);
  document.querySelectorAll("#theme-switch [data-theme-opt]").forEach((btn) => {
    btn.classList.toggle("active", btn.getAttribute("data-theme-opt") === norm);
  });
  if (persist) { invoke("set_theme", { theme: norm }).catch(() => {}); }
}

function renderAuth() {
  if (loggedIn) {
    show(d.logoutBtn); show(d.syncBtn); hide(d.welcome);
    d.statusBadge.textContent = t("auth.online"); d.statusBadge.classList.add("badge-online"); show(d.statusBadge);
  } else {
    hide(d.logoutBtn); show(d.welcome);
    d.statusBadge.textContent = t("auth.offline"); d.statusBadge.classList.remove("badge-online"); show(d.statusBadge);
  }
}

async function doSetToken() {
  hide(d.loginError);
  d.loginBtn.disabled = true; d.loginBtn.textContent = t("auth.opening");
  try {
    await invoke("login_via_www");
  } catch (e) {
    d.loginError.textContent = t("error.prefix") + ": " + String(e); show(d.loginError);
  } finally {
    d.loginBtn.disabled = false; d.loginBtn.textContent = t("welcome.login");
  }
}

async function doLogout() {
  try {
    await invoke("logout"); loggedIn = false; gamesCache = [];
    renderAuth(); d.gameGrid.innerHTML = "";
  } catch (e) { console.error(e); }
}

async function sync() {
  if (!loggedIn) return;
  show(d.syncBar);
  try { await invoke("sync_library"); await loadGames(); }
  catch (e) {
    console.error("sync:", e);
    if (String(e).toLowerCase().includes("auth")) { loggedIn = false; renderAuth(); }
    else {
      d.gameEmpty.innerHTML = `<p class="error">${t("library.loadError")}: ${esc(String(e))}</p>`;
      show(d.gameEmpty); show(d.gameSection);
    }
  }
  finally { hide(d.syncBar); }
}

async function loadGames() {
  let games;
  try { games = await invoke("get_games"); } catch (e) { games = []; }
  gamesCache = Array.isArray(games) ? games : [];
  renderGames();
}

/** Rysuje siatkę gier (filtr wyszukiwarki + filtr platformy). */
function renderGames() {
  const q = searchQuery;
  // Filtr platformy stosujemy tylko wtedy, gdy API dostarczyło dane o platformach.
  const usePlatform = platformFilter !== "all" && platformDataAvailable();
  const list = gamesCache.filter((g) => {
    if (q && !(g.title || "").toLowerCase().includes(q)) return false;
    if (usePlatform && !(Array.isArray(g.platforms) && g.platforms.includes(platformFilter))) return false;
    return true;
  });
  d.gameGrid.innerHTML = "";
  if (gamesCache.length === 0) {
    d.gameCount.textContent = "";
    d.gameEmpty.innerHTML = `<p>${t("library.emptyPrefix")}<strong>${t("library.emptyAction")}</strong>${t("library.emptySuffix")}</p>`;
    show(d.gameEmpty); hide(d.gameSection);
    return;
  }
  show(d.gameSection);
  d.gameCount.textContent = String(list.length);
  if (list.length === 0) {
    d.gameEmpty.innerHTML = `<p>${t("library.noResults")}</p>`;
    show(d.gameEmpty);
    return;
  }
  hide(d.gameEmpty);
  for (const g of list) {
    const card = document.createElement("div");
    card.className = "game-card"; card.dataset.gameId = g.id;
    const src = g.imageUrl || g.iconUrl;
    const img = src
      ? `<img class="game-card-cover" src="${esc(src)}" alt="${esc(g.title)}" loading="lazy">`
      : `<div class="game-card-fallback">\uD83C\uDFAE</div>`;
    const genre = g.genre ? `<div class="game-card-genre">${esc(g.genre)}</div>` : "";
    card.innerHTML = `${img}${genre}${platformBadgesHtml(g.platforms)}` +
      `<div class="game-card-foot"><div class="game-card-title">${esc(g.title)}</div>${gameStateTagHtml(g.id)}</div>`;
    card.addEventListener("click", () => openGame(g));
    d.gameGrid.appendChild(card);
  }
}

/** Czy którykolwiek wiersz biblioteki zawiera informację o platformach. */
function platformDataAvailable() {
  return gamesCache.some((g) => Array.isArray(g.platforms) && g.platforms.length);
}

/** Stan plików gry wyliczony z wierszy pobierań: `downloading` / `downloaded` / `missing`. */
function gameState(gameId) {
  const rows = [...dlRows.values()].filter((r) => String(r.gameId) === String(gameId));
  if (rows.some((r) => r.status === "downloading" || r.status === "pending")) return "downloading";
  if (rows.some((r) => r.status === "completed" && r.fileExists)) return "downloaded";
  if (rows.some((r) => r.status === "completed")) return "missing";
  return "";
}

/** Tag stanu gry na kafelku biblioteki (Pobrane / W trakcie pobierania / Brak pliku). */
function gameStateTagHtml(gameId) {
  const st = gameState(gameId);
  if (!st) return "";
  const key = { downloading: "game.state.downloading", downloaded: "game.state.downloaded", missing: "game.state.missing" }[st];
  return `<span class="game-state ${st}">${esc(t(key))}</span>`;
}

/** Znaczniki platform (Win/Lin/Mac) w rogu kafelka gry. */
function platformBadgesHtml(platforms) {
  if (!Array.isArray(platforms) || !platforms.length) return "";
  const parts = [];
  if (platforms.includes("windows")) parts.push(`<span class="plat-tag plat-windows">Win</span>`);
  if (platforms.includes("linux")) parts.push(`<span class="plat-tag plat-linux">Lin</span>`);
  if (platforms.includes("macos")) parts.push(`<span class="plat-tag plat-macos">Mac</span>`);
  return parts.length ? `<div class="game-card-platforms">${parts.join("")}</div>` : "";
}

/** Ustawia aktywny filtr platformy w bibliotece i odświeża siatkę. */
function selectPlatformFilter(platform) {
  platformFilter = platform || "all";
  if (d.platformFilter) {
    d.platformFilter.querySelectorAll("[data-platform]").forEach((b) =>
      b.classList.toggle("active", b.getAttribute("data-platform") === platformFilter));
  }
  renderGames();
}

async function openGame(game) {
  currentGameId = String(game.id);
  const image = game.imageUrl || game.iconUrl;
  d.modalTitle.textContent = game.title;
  if (image) { d.modalHeroImg.src = image; d.modalHeroImg.classList.remove("hidden"); }
  else { d.modalHeroImg.removeAttribute("src"); d.modalHeroImg.classList.add("hidden"); }
  d.modalBody.innerHTML = `<p class="loading">${t("modal.loading")}</p>`;
  closeScreenshotViewer();
  switchModalTab("info");
  d.modal.showModal();
  // Metadane (opis, producenci, wydawcy, gatunki) + instalatory — równolegle.
  const [metaRes, instRes] = await Promise.allSettled([
    invoke("get_game_details", { gameId: game.id, language: currentLang() }),
    invoke("get_installers", { gameId: game.id }),
  ]);
  const meta = metaRes.status === "fulfilled" ? metaRes.value : null;
  const installers = instRes.status === "fulfilled" ? instRes.value : null;
  const instError = instRes.status === "rejected" ? instRes.reason : null;
  modalInstallers = Array.isArray(installers) ? installers : [];
  installerFilter = "all";
  renderModal(game, meta, installers, instError);
}

/**
 * Buduje treść modala w dwóch zakładkach:
 *  - „info” — metadane, opis oraz zrzuty ekranu (jeśli API je zwraca),
 *  - „installers” — lista instalatorów do pobrania (filtr i sortowanie wg platformy).
 */
function renderModal(game, meta, installers, installersError) {
  const devs = (meta && meta.developers && meta.developers.length) ? meta.developers : (game.developers || []);
  const pubs = (meta && meta.publishers && meta.publishers.length) ? meta.publishers : (game.publishers || []);
  const genres = (meta && meta.genres && meta.genres.length) ? meta.genres : (game.genre ? [game.genre] : []);
  const description = (meta && meta.description) || "";
  const screenshots = (meta && Array.isArray(meta.screenshots)) ? meta.screenshots : [];

  const metaRows = [];
  if (genres.length) metaRows.push(metaRow("modal.genre", genres.join(", ")));
  if (devs.length) metaRows.push(metaRow("modal.developer", devs.join(", ")));
  if (pubs.length) metaRows.push(metaRow("modal.publisher", pubs.join(", ")));
  if (game.releaseDate) metaRows.push(metaRow("modal.releaseDate", dateOnly(game.releaseDate)));

  // ---- zakładka „Opis” ----
  let info = "";
  if (metaRows.length) info += `<div class="modal-meta">${metaRows.join("")}</div>`;
  if (description) info += `<p class="modal-description">${esc(description)}</p>`;
  if (screenshots.length) {
    info += `<h3 class="modal-section-title">${t("modal.screenshots")}</h3>`;
    info += `<div class="screenshot-grid">${screenshots.map(screenshotHtml).join("")}</div>`;
  }
  if (!info) info = `<p class="empty-state">${t("modal.noDescription")}</p>`;

  // ---- zakładka „Instalatory” ----
  const hasInstallers = !installersError && installers && installers.length;
  let inst = "";
  if (installersError) {
    inst = `<p class="error">${t("error.prefix")}: ${esc(String(installersError))}</p>`;
  } else if (!installers || !installers.length) {
    inst = `<p class="empty-state">${t("modal.noInstallers")}</p>`;
  } else {
    inst = gameDirRowHtml(game.id) + `<div class="platform-filter" id="inst-filter">` +
      platformChip("all", t("filter.all")) +
      platformChip("windows", t("filter.windows")) +
      platformChip("linux", t("filter.linux")) +
      platformChip("macos", t("filter.macos")) +
      `</div><div id="inst-list" class="installer-list"></div>`;
  }

  d.modalBody.innerHTML =
    `<div class="modal-panel" data-panel="info">${info}</div>` +
    `<div class="modal-panel hidden" data-panel="installers">${inst}</div>`;

  if (hasInstallers) {
    renderInstallerList(game.id);
    d.modalBody.querySelector("#inst-filter").addEventListener("click", (e) => {
      const btn = e.target.closest("[data-platform]");
      if (!btn) return;
      installerFilter = btn.getAttribute("data-platform") || "all";
      d.modalBody.querySelector("#inst-filter").querySelectorAll("[data-platform]").forEach((b) =>
        b.classList.toggle("active", b === btn));
      renderInstallerList(game.id);
    });
  }

  bindGameDirButton(game.id);

  // Podgląd zrzutów ekranu
  d.modalBody.querySelectorAll(".screenshot-thumb").forEach((img) =>
    img.addEventListener("click", () => openScreenshotViewer(img.dataset.full)));

  closeScreenshotViewer();
  switchModalTab(activeModalTab);
}

/** Przycisk filtra platformy (z podświetleniem aktywnego wyboru instalatorów). */
function platformChip(value, label) {
  const active = installerFilter === value ? " active" : "";
  return `<button class="chip${active}" data-platform="${value}">${esc(label)}</button>`;
}

/** Rysuje listę instalatorów w modalu (filtr + sortowanie wg platformy). */
function renderInstallerList(gameId) {
  const list = d.modalBody.querySelector("#inst-list");
  if (!list) return;
  const items = sortInstallers(modalInstallers)
    .filter((i) => installerFilter === "all" || normOs(i.os) === installerFilter);
  if (!items.length) {
    list.innerHTML = `<p class="empty-state">${t("modal.noInstallersForPlatform")}</p>`;
    return;
  }
  list.innerHTML = items.map((i) => installerItemHtml(i, gameId)).join("");
  list.querySelectorAll(".dl-t").forEach((b) =>
    b.addEventListener("click", () => startDownload(parseInt(b.dataset.game), b.dataset.inst, b)));
}

/** Sortuje instalatory: Windows → Linux → macOS → inne, a następnie po nazwie. */
function sortInstallers(items) {
  const rank = (os) => ({ windows: 0, linux: 1, macos: 2 }[normOs(os)] ?? 3);
  return [...items].sort((a, b) =>
    rank(a.os) - rank(b.os) || String(a.name || "").localeCompare(String(b.name || "")));
}

/** Normalizuje oznaczenie systemu instalatora do `"windows"` / `"linux"` / `"macos"`. */
function normOs(os) {
  const s = String(os || "").toLowerCase();
  if (s.startsWith("win")) return "windows";
  if (s.startsWith("lin")) return "linux";
  if (s.startsWith("mac") || s.startsWith("osx") || s.startsWith("darwin")) return "macos";
  return s;
}

/** HTML pojedynczego zrzutu ekranu (miniatura; klik otwiera podgląd). */
function screenshotHtml(s) {
  const thumb = s.thumb || s.full || "";
  const full = s.full || s.thumb || "";
  return `<img class="screenshot-thumb" src="${esc(thumb)}" data-full="${esc(full)}" alt="" loading="lazy">`;
}

/** Przełącza aktywną zakładkę modala i pokazuje odpowiadający jej panel. */
function switchModalTab(name) {
  activeModalTab = name === "installers" ? "installers" : "info";
  d.modalTabs.querySelectorAll(".modal-tab").forEach((b) =>
    b.classList.toggle("active", b.getAttribute("data-tab") === activeModalTab));
  d.modalBody.querySelectorAll(".modal-panel").forEach((p) =>
    p.classList.toggle("hidden", p.getAttribute("data-panel") !== activeModalTab));
}

/** Otwiera podgląd zrzutu ekranu w pełnym rozmiarze. */
function openScreenshotViewer(url) {
  if (!url) return;
  d.screenshotViewerImg.src = url;
  show(d.screenshotViewer);
}

/** Zamyka podgląd zrzutu ekranu. */
function closeScreenshotViewer() {
  hide(d.screenshotViewer);
  d.screenshotViewerImg.removeAttribute("src");
}

/** Pojedyncza metadana (etykieta + wartość) w nagłówku modala. */
function metaRow(labelKey, value) {
  return `<span class="meta-item"><span class="meta-label">${t(labelKey)}</span>${esc(value)}</span>`;
}

/** Zwraca samą datę (RRRR-MM-DD), obcinając ewentualną godzinę/strefę. */
function dateOnly(s) {
  const str = String(s == null ? "" : s);
  const m = str.match(/^\d{4}-\d{2}-\d{2}/);
  return m ? m[0] : str;
}

/** Wiersz z folderem docelowym gry + przycisk zmiany folderu (zakładka „Instalatory”). */
function gameDirRowHtml(gameId) {
  const g = gamesCache.find((x) => String(x.id) === String(gameId));
  const own = !!(g && g.installDir);
  const dir = own ? g.installDir : currentDir;
  return `<div class="game-dir-row">` +
    `<div class="game-dir-info">` +
    `<span class="game-dir-label">${t("game.folder")}</span>` +
    `<span class="path-chip" title="${esc(dir)}">${esc(dir || "—")}</span>` +
    (own ? "" : `<span class="game-dir-hint">${t("game.folderHint")}</span>`) +
    `</div>` +
    (dir ? `<button class="btn btn-small" id="btn-open-game-dir" data-game="${gameId}">${t("folder.open")}</button>` : "") +
    `<button class="btn btn-small" id="btn-game-dir" data-game="${gameId}">${t("game.folder.change")}</button>` +
    `</div>`;
}

/** Podpina przycisk zmiany folderu gry (ponownie po każdym przerysowaniu wiersza). */
function bindGameDirButton(gameId) {
  const btn = d.modalBody.querySelector("#btn-game-dir");
  if (btn) btn.addEventListener("click", () => pickGameDir(gameId));
  const openBtn = d.modalBody.querySelector("#btn-open-game-dir");
  if (openBtn) openBtn.addEventListener("click", () => openGameFolder(gameId));
}

/** Otwiera podaną ścieżkę (folder lub plik) w systemowym menedżerze plików. */
async function openFolder(path) {
  if (!path) return;
  try {
    await invoke("open_folder", { path });
  } catch (e) {
    console.error("open folder:", e);
  }
}

/** Otwiera folder pobrania z listy pobierań (folder pliku instalatora). */
function openDlFolder(id) {
  const row = dlRows.get(id);
  if (row) openFolder(row.targetPath);
}

/** Otwiera folder wybrany dla gry (lub domyślny, gdy gra nie ma jeszcze własnego). */
function openGameFolder(gameId) {
  const g = gamesCache.find((x) => String(x.id) === String(gameId));
  openFolder((g && g.installDir) || currentDir);
}

/** Otwiera wybór folderu dla gry i zapisuje go; zwraca ścieżkę lub `null` (anulowano). */
async function pickGameDir(gameId) {
  try {
    const dir = await invoke("pick_game_dir", { gameId: Number(gameId) });
    if (!dir) return null;
    const g = gamesCache.find((x) => String(x.id) === String(gameId));
    if (g) g.installDir = dir;
    const row = d.modalBody.querySelector(".game-dir-row");
    if (row) { row.outerHTML = gameDirRowHtml(gameId); bindGameDirButton(gameId); }
    return dir;
  } catch (e) {
    console.error("pick game dir:", e);
    return null;
  }
}

/** Folder gry — jeśli gra go nie ma, pyta o niego i zapamiętuje. */
async function ensureGameDir(gameId) {
  const g = gamesCache.find((x) => String(x.id) === String(gameId));
  if (g && g.installDir) return g.installDir;
  return pickGameDir(gameId);
}

/** Stan instalatora w danej grze (na podstawie wierszy pobierań). */
function installerRowState(gameId, installerId) {
  const rows = [...dlRows.values()].filter((r) =>
    String(r.gameId) === String(gameId) && r.installerId === installerId);
  if (!rows.length) return "";
  if (rows.some((r) => r.status === "downloading" || r.status === "pending")) return "downloading";
  const done = rows.filter((r) => r.status === "completed");
  if (done.length && done.every((r) => r.fileExists)) return "downloaded";
  if (done.length) return "missing";
  if (rows.some((r) => r.status === "paused")) return "paused";
  return "";
}

/** Tag stanu przy pojedynczym instalatorze. */
function installerStateTagHtml(gameId, installerId) {
  const st = installerRowState(gameId, installerId);
  if (!st) return "";
  const key = {
    downloading: "game.state.downloading", downloaded: "game.state.downloaded",
    missing: "game.state.missing", paused: "status.paused",
  }[st];
  return `<span class="inst-state ${st}">${esc(t(key))}</span>`;
}

/** HTML pojedynczego instalatora na liście w modalu. */
function installerItemHtml(i, gameId) {
  return `
    <div class="installer-item">
      <div class="installer-main">
        <h4>${esc(i.name)}</h4>
        <div class="installer-meta">
          <span class="tag">${esc((i.os || "").toUpperCase())}</span>
          <span>${fmtSize(i.size)}</span>
          <span>v${esc(i.version || "?")}</span>
          <span>${esc(i.language || "en")}</span>
          ${installerStateTagHtml(gameId, i.id)}
        </div>
      </div>
      <button class="btn btn-primary dl-t" data-game="${gameId}" data-inst="${esc(i.id)}">${t("modal.download")}</button>
    </div>`;
}

function closeModal() { closeScreenshotViewer(); d.modal.close(); }

/* ------------------------------- AMAZON ------------------------------- */

/** Stan integracji z Amazon Games: czy jest nile i czy jesteśmy zalogowani. */
let amazonStatus = null;
/** Gry z biblioteki Amazon Games. */
let amazonGames = [];
/** Trwające instalacje Amazon: id gry -> { title, percent, line, ok, error }. */
const amazonActive = new Map();
/** Czy czekamy na wklejenie adresu przekierowania po zalogowaniu w przeglądarce. */
let amazonLoginPending = false;

/** Odświeża status nile i (opcjonalnie) wymusza synchronizację biblioteki Amazon. */
async function loadAmazon(syncFirst = false) {
  try { amazonStatus = await invoke("amazon_status"); }
  catch (e) { amazonStatus = { available: false, loggedIn: false, username: null }; }
  renderAmazonStatus();
  if (!amazonStatus.available || !amazonStatus.loggedIn) return;
  if (syncFirst) {
    const label = d.amazonRefreshBtn.textContent;
    d.amazonRefreshBtn.disabled = true;
    d.amazonRefreshBtn.textContent = t("amazon.syncing");
    try { await invoke("amazon_sync"); } catch (e) { console.error("amazon sync:", e); }
    d.amazonRefreshBtn.disabled = false;
    d.amazonRefreshBtn.textContent = label;
  }
  try { amazonGames = await invoke("amazon_library"); }
  catch (e) { amazonGames = []; console.error("amazon library:", e); }
  renderAmazonGames();
}

/** Rysuje pasek statusu (brak nile / logowanie / konto) i stan pusty. */
function renderAmazonStatus() {
  const st = amazonStatus;
  if (!st || !st.available) {
    show(d.amazonSetup); hide(d.amazonBar); hide(d.amazonGrid); hide(d.amazonEmpty);
    hide(d.amazonLoginPanel);
    if (st) d.amazonSetupMsg.textContent = t("amazon.missing");
    return;
  }
  hide(d.amazonSetup); show(d.amazonBar);
  d.amazonLoginPanel.classList.toggle("hidden", !amazonLoginPending);
  d.amazonUser.textContent = st.loggedIn ? (st.username || t("amazon.account")) : t("amazon.notLoggedIn");
  d.amazonUser.classList.toggle("badge-online", !!st.loggedIn);
  d.amazonLoginBtn.classList.toggle("hidden", !!st.loggedIn);
  d.amazonLogoutBtn.classList.toggle("hidden", !st.loggedIn);
  d.amazonRefreshBtn.classList.toggle("hidden", !st.loggedIn);
  if (!st.loggedIn) {
    hide(d.amazonGrid);
    d.amazonEmpty.innerHTML = `<p>${t("amazon.loginHint")}</p>`;
    show(d.amazonEmpty);
  }
}

/** Kafelek gry Amazon: okładka zastępcza, tytuł, stan i akcje. */
function amazonCardHtml(g) {
  const initial = (g.title || "?").trim().charAt(0).toUpperCase();
  const installed = g.installed && g.dirExists;
  const badge = installed ? `<span class="game-state downloaded">${esc(t("amazon.installed"))}</span>` : "";
  const primary = installed
    ? `<button class="btn btn-small btn-primary" data-amazon-launch="${esc(g.id)}">${esc(t("amazon.play"))}</button>`
    : `<button class="btn btn-small btn-primary" data-amazon-install="${esc(g.id)}">${esc(t("amazon.install"))}</button>`;
  const open = g.installDir
    ? `<button class="btn btn-small" data-amazon-open="${esc(g.installDir)}">${esc(t("folder.open"))}</button>`
    : "";
  // Okładka z biblioteki Amazon; gdy obrazek nie wczyta się, zostaje sama litera.
  const cover = g.coverUrl
    ? `<img src="${esc(g.coverUrl)}" alt="" loading="lazy" onerror="this.remove()">`
    : "";
  return `<div class="game-card amazon-card">
    <div class="amazon-cover" aria-hidden="true"><span>${esc(initial)}</span>${cover}</div>
    ${g.genre ? `<div class="game-card-genre">${esc(g.genre)}</div>` : ""}
    <div class="game-card-foot"><div class="game-card-title">${esc(g.title)}</div>${badge}</div>
    <div class="amazon-card-actions">${primary}${open}
      <button class="btn btn-small" data-amazon-dir="${esc(g.id)}">${esc(t("amazon.folder"))}</button>
    </div>
  </div>`;
}

/** Rysuje siatkę gier Amazon (albo komunikat o pustej bibliotece). */
function renderAmazonGames() {
  if (!amazonStatus || !amazonStatus.available || !amazonStatus.loggedIn) return;
  if (!amazonGames.length) {
    d.amazonGrid.innerHTML = ""; hide(d.amazonGrid);
    d.amazonEmpty.innerHTML = `<p>${t("amazon.empty")}</p>`;
    show(d.amazonEmpty);
    return;
  }
  hide(d.amazonEmpty);
  show(d.amazonGrid);
  d.amazonGrid.innerHTML = amazonGames.map(amazonCardHtml).join("");
}

/**
 * Lista trwających/zakończonych instalacji Amazon z postępem.
 * Rysowana w zakładce Pobierania (wspólnej z pobraniami GOG), nie w Prime Gaming.
 */
function renderStoreDownloads() {
  const rows = [
    ...[...amazonActive.entries()].map(([id, a]) => ({ id, store: "prime", ...a })),
    ...[...epicActive.entries()].map(([id, a]) => ({ id, store: "epic", ...a })),
  ];
  if (!rows.length) {
    d.dlStores.innerHTML = "";
    hide(d.dlStores);
    // Gdy nie ma już żadnych pobrań, przywracamy komunikat o pustej liście.
    if (!dlRows.size) renderDownloads();
    return;
  }
  show(d.dlStores);
  d.dlStores.innerHTML = rows.map((a) => {
    const pct = a.percent == null ? null : Math.min(a.percent, 100);
    const cls = a.ok === true ? "completed" : a.ok === false ? "failed" : "active";
    const label = a.ok === true ? t("amazon.done") : a.ok === false ? t("amazon.failed") : t("status.downloading");
    const store = a.store === "epic" ? t("epic.short") : t("amazon.short");
    const bar = pct == null ? "" :
      `<div class="dl-progress-track"><div class="dl-progress-fill ${cls}" style="width:${pct}%"></div></div>` +
      `<div class="dl-info"><span class="dl-pct">${pct.toFixed(1)}%</span></div>`;
    return `<div class="download-item" data-store-active="${esc(a.store)}">
      <div class="dl-head">
        <div class="dl-title">${esc(a.title)} <span class="dl-store">${esc(store)}</span></div>
        <div class="dl-status ${cls}">${esc(label)}</div>
      </div>
      ${bar}
      ${a.error ? `<div class="dl-error">${esc(a.error)}</div>` : `<div class="dl-path">${esc(a.line || "")}</div>`}
    </div>`;
  }).join("");
}

/** Wszystkie trzy widoki Amazon — używane m.in. przy zmianie języka. */
function renderAmazonUi() {
  renderAmazonStatus();
  renderAmazonGames();
  renderStoreDownloads();
}

/** Pokazuje komunikat błędu pod polem adresu logowania. */
function amazonLoginError(msg) {
  d.amazonLoginError.textContent = msg;
  show(d.amazonLoginError);
}

/**
 * Logowanie konta Amazon.
 * nile w trybie `--non-interactive` wypisuje adres logowania i kończy działanie
 * (interaktywne `nile auth --login` czytałoby dane z terminala i kończyło się EOFError),
 * więc backend otwiera stronę w przeglądarce, a my prosimy o wklejenie adresu powrotnego.
 */
async function amazonLogin() {
  d.amazonLoginBtn.disabled = true;
  d.amazonLoginBtn.textContent = t("amazon.loginOpening");
  hide(d.amazonLoginError);
  try {
    await invoke("amazon_login");
    amazonLoginPending = true;
    d.amazonLoginUrl.value = "";
    show(d.amazonLoginPanel);
    d.amazonLoginUrl.focus();
  } catch (e) {
    d.amazonEmpty.innerHTML = `<p class="error">${t("error.prefix")}: ${esc(String(e))}</p>`;
    show(d.amazonEmpty);
  } finally {
    d.amazonLoginBtn.disabled = false;
    d.amazonLoginBtn.textContent = t("amazon.login");
  }
  renderAmazonStatus();
}

/** Kończy logowanie na podstawie adresu, na który przekierowała przeglądarka. */
async function amazonLoginFinish() {
  const url = d.amazonLoginUrl.value.trim();
  hide(d.amazonLoginError);
  if (!url) { amazonLoginError(t("amazon.loginNoUrl")); return; }
  const btn = d.amazonLoginFinishBtn;
  const label = btn.textContent;
  btn.disabled = true;
  btn.textContent = t("amazon.loginFinishing");
  let ok = false;
  try {
    amazonStatus = await invoke("amazon_login_complete", { redirect: url });
    ok = true;
    amazonLoginPending = false;
    d.amazonLoginUrl.value = "";
    hide(d.amazonLoginPanel);
    renderAmazonStatus();
  } catch (e) {
    amazonLoginError(`${t("error.prefix")}: ${String(e)}`);
  } finally {
    btn.disabled = false;
    btn.textContent = label;
  }
  if (ok) await loadAmazon(true);
}

/** Anuluje oczekiwanie na adres przekierowania. */
function amazonLoginCancel() {
  amazonLoginPending = false;
  d.amazonLoginUrl.value = "";
  hide(d.amazonLoginError);
  hide(d.amazonLoginPanel);
  renderAmazonStatus();
}

/** Wylogowanie konta Amazon. */
async function amazonLogout() {
  try { await invoke("amazon_logout"); } catch (e) { console.error("amazon logout:", e); }
  amazonGames = [];
  d.amazonGrid.innerHTML = "";
  await loadAmazon();
}

/** Kopiuje komendę instalacji nile do schowka. */
async function amazonCopyCommand() {
  const cmd = "winget install --id imLinguin.nile";
  try {
    await navigator.clipboard.writeText(cmd);
    const label = d.amazonCopyBtn.textContent;
    d.amazonCopyBtn.textContent = t("amazon.copied");
    setTimeout(() => { d.amazonCopyBtn.textContent = label; }, 1500);
  } catch (e) { console.error("clipboard:", e); }
}

/** Instaluje nile przez winget (bez terminala) i odświeża status integracji. */
async function amazonInstallNile() {
  const btn = d.amazonInstallNileBtn;
  const label = btn.textContent;
  btn.disabled = true;
  btn.textContent = t("amazon.installingNile");
  hide(d.amazonSetupError);
  d.amazonSetupError.textContent = "";
  d.amazonSetupProgress.textContent = t("amazon.nileProgress");
  show(d.amazonSetupProgress);
  try {
    await invoke("amazon_install_nile");
  } catch (e) {
    d.amazonSetupError.textContent = `${t("error.prefix")}: ${String(e)}`;
    show(d.amazonSetupError);
  } finally {
    btn.disabled = false;
    btn.textContent = label;
    hide(d.amazonSetupProgress);
  }
  await loadAmazon();
}

/** Wybór folderu instalacji dla gry Amazon. */
async function amazonPickDir(id) {
  let dir = null;
  try { dir = await invoke("pick_amazon_dir", { gameId: id }); }
  catch (e) { console.error("pick amazon dir:", e); return; }
  if (!dir) return;
  const g = amazonGames.find((x) => x.id === id);
  if (g) g.installDir = dir;
  renderAmazonGames();
}

/** Pobiera i instaluje grę Amazon (przez nile); postęp przychodzi zdarzeniami. */
async function amazonInstall(id) {
  const running = amazonActive.get(id);
  if (running && running.ok == null) return; // już się instaluje
  const g = amazonGames.find((x) => x.id === id);
  if (!g) return;
  if (!g.installDir) {
    await amazonPickDir(id);
    if (!g.installDir) return; // anulowano wybór folderu
  }
  amazonActive.set(id, { title: g.title, percent: null, line: t("amazon.starting"), ok: null, error: "" });
  renderStoreDownloads();
  try {
    await invoke("amazon_install", { gameId: id });
    // Postęp jest w zakładce Pobierania — od razu ją pokazujemy (jak przy GOG).
    switchView("downloads");
  } catch (e) {
    amazonActive.set(id, { title: g.title, percent: null, line: "", ok: false, error: String(e) });
    renderStoreDownloads();
  }
}

/** Uruchamia zainstalowaną grę Amazon. */
async function amazonLaunch(id) {
  const g = amazonGames.find((x) => x.id === id);
  try {
    await invoke("amazon_launch", { gameId: id });
    amazonActive.delete(id);
    renderStoreDownloads();
  } catch (e) {
    amazonActive.set(id, { title: g ? g.title : id, percent: null, line: "", ok: false, error: String(e) });
    renderStoreDownloads();
  }
}

/** Obsługa kliknięć na kafelkach Amazon (delegacja — kafelki są przerysowywane). */
async function onAmazonGridClick(e) {
  const launch = e.target.closest("[data-amazon-launch]");
  if (launch) { await amazonLaunch(launch.getAttribute("data-amazon-launch")); return; }
  const install = e.target.closest("[data-amazon-install]");
  if (install) { await amazonInstall(install.getAttribute("data-amazon-install")); return; }
  const dir = e.target.closest("[data-amazon-dir]");
  if (dir) { await amazonPickDir(dir.getAttribute("data-amazon-dir")); return; }
  const open = e.target.closest("[data-amazon-open]");
  if (open) openFolder(open.getAttribute("data-amazon-open"));
}

/* ---------------------------- EPIC GAMES ---------------------------- */

let epicStatus = null;
/** Biblioteka Epic (`legendary list --json`). */
let epicGames = [];
/** Trwające/zakończone instalacje Epic: app_name -> { title, percent, line, ok, error }. */
const epicActive = new Map();
/** Czy czekamy na kod logowania wklejony ze strony legendary.gl/epiclogin. */
let epicLoginPending = false;

/** Wczytuje status integracji i bibliotekę Epic; `sync` wymusza pobranie metadanych. */
async function loadEpic(sync) {
  try { epicStatus = await invoke("epic_status"); }
  catch (e) { epicStatus = { available: false, loggedIn: false, username: null }; }
  renderEpicStatus();
  if (!epicStatus.available || !epicStatus.loggedIn) {
    epicGames = [];
    renderEpicGames();
    return;
  }
  if (sync) {
    const btn = d.epicRefreshBtn;
    const label = btn.textContent;
    btn.disabled = true;
    btn.textContent = t("amazon.syncing");
    try { await invoke("epic_sync"); } catch (e) { console.error("epic sync:", e); }
    btn.disabled = false;
    btn.textContent = label;
  }
  try { epicGames = await invoke("epic_library"); }
  catch (e) { epicGames = []; console.error("epic library:", e); }
  renderEpicGames();
}

/** Rysuje pasek statusu (brak legendary / logowanie / konto) i stan pusty. */
function renderEpicStatus() {
  const st = epicStatus;
  if (!st || !st.available) {
    show(d.epicSetup); hide(d.epicBar); hide(d.epicGrid); hide(d.epicEmpty);
    hide(d.epicLoginPanel);
    if (st) d.epicSetupMsg.textContent = t("epic.missing");
    return;
  }
  hide(d.epicSetup); show(d.epicBar);
  d.epicLoginPanel.classList.toggle("hidden", !epicLoginPending);
  d.epicUser.textContent = st.loggedIn ? (st.username || t("epic.account")) : t("epic.notLoggedIn");
  d.epicUser.classList.toggle("badge-online", !!st.loggedIn);
  d.epicLoginBtn.classList.toggle("hidden", !!st.loggedIn);
  d.epicLogoutBtn.classList.toggle("hidden", !st.loggedIn);
  d.epicRefreshBtn.classList.toggle("hidden", !st.loggedIn);
  if (!st.loggedIn) {
    hide(d.epicGrid);
    d.epicEmpty.innerHTML = `<p>${t("epic.loginHint")}</p>`;
    d.epicEmpty.classList.remove("hidden");
  }
}

/** Kafelek gry Epic: okładka z metadanych, tytuł, stan i akcje. */
function epicCardHtml(g) {
  const initial = (g.title || "?").trim().charAt(0).toUpperCase();
  const installed = g.installed && g.dirExists;
  const badge = installed ? `<span class="game-state downloaded">${esc(t("epic.installed"))}</span>` : "";
  const primary = installed
    ? `<button class="btn btn-small btn-primary" data-epic-launch="${esc(g.id)}">${esc(t("epic.play"))}</button>`
    : `<button class="btn btn-small btn-primary" data-epic-install="${esc(g.id)}">${esc(t("epic.install"))}</button>`;
  const open = g.installDir
    ? `<button class="btn btn-small" data-epic-open="${esc(g.installDir)}">${esc(t("folder.open"))}</button>`
    : "";
  // Okładka z biblioteki Epic; gdy obrazek nie wczyta się, zostaje sama litera.
  const cover = g.coverUrl
    ? `<img src="${esc(g.coverUrl)}" alt="" loading="lazy" onerror="this.remove()">`
    : "";
  return `<div class="game-card amazon-card">
    <div class="amazon-cover" aria-hidden="true"><span>${esc(initial)}</span>${cover}</div>
    ${g.developer ? `<div class="game-card-genre">${esc(g.developer)}</div>` : ""}
    <div class="game-card-foot"><div class="game-card-title">${esc(g.title)}</div>${badge}</div>
    <div class="amazon-card-actions">${primary}${open}
      <button class="btn btn-small" data-epic-dir="${esc(g.id)}">${esc(t("epic.folder"))}</button>
    </div>
  </div>`;
}

/** Rysuje siatkę gier Epic (albo komunikat o pustej bibliotece). */
function renderEpicGames() {
  if (!epicStatus || !epicStatus.available || !epicStatus.loggedIn) return;
  if (!epicGames.length) {
    d.epicGrid.innerHTML = ""; hide(d.epicGrid);
    d.epicEmpty.innerHTML = `<p>${t("epic.empty")}</p>`;
    show(d.epicEmpty);
    return;
  }
  hide(d.epicEmpty);
  show(d.epicGrid);
  d.epicGrid.innerHTML = epicGames.map(epicCardHtml).join("");
}

/** Wszystkie widoki Epic — używane m.in. przy zmianie języka. */
function renderEpicUi() {
  renderEpicStatus();
  renderEpicGames();
  renderStoreDownloads();
}

/** Pokazuje komunikat błędu pod polem kodu logowania. */
function epicLoginError(msg) {
  d.epicLoginError.textContent = msg;
  show(d.epicLoginError);
}

/**
 * Start logowania Epic.
 * `legendary auth` bez argumentów czyta kod z terminala (`input()`), co bez konsoli kończy
 * się `EOFError`. Dlatego otwieramy stałą stronę logowania, a kod wkleja użytkownik.
 */
async function epicLogin() {
  d.epicLoginBtn.disabled = true;
  d.epicLoginBtn.textContent = t("epic.loginOpening");
  hide(d.epicLoginError);
  try {
    await invoke("epic_login");
    epicLoginPending = true;
    d.epicLoginCode.value = "";
    show(d.epicLoginPanel);
    d.epicLoginCode.focus();
  } catch (e) {
    d.epicEmpty.innerHTML = `<p class="error">${t("error.prefix")}: ${esc(String(e))}</p>`;
    show(d.epicEmpty);
  } finally {
    d.epicLoginBtn.disabled = false;
    d.epicLoginBtn.textContent = t("epic.login");
  }
  renderEpicStatus();
}

/** Kończy logowanie kodem (albo całym JSON-em) ze strony logowania. */
async function epicLoginFinish() {
  const code = d.epicLoginCode.value.trim();
  hide(d.epicLoginError);
  if (!code) { epicLoginError(t("epic.loginNoCode")); return; }
  const btn = d.epicLoginFinishBtn;
  const label = btn.textContent;
  btn.disabled = true;
  btn.textContent = t("amazon.loginFinishing");
  let ok = false;
  try {
    epicStatus = await invoke("epic_login_complete", { code });
    ok = true;
    epicLoginPending = false;
    d.epicLoginCode.value = "";
    hide(d.epicLoginPanel);
    renderEpicStatus();
  } catch (e) {
    epicLoginError(`${t("error.prefix")}: ${String(e)}`);
  } finally {
    btn.disabled = false;
    btn.textContent = label;
  }
  if (ok) await loadEpic(true);
}

/** Anuluje oczekiwanie na kod logowania. */
function epicLoginCancel() {
  epicLoginPending = false;
  d.epicLoginCode.value = "";
  hide(d.epicLoginError);
  hide(d.epicLoginPanel);
  renderEpicStatus();
}

/** Wylogowanie konta Epic. */
async function epicLogout() {
  try { await invoke("epic_logout"); } catch (e) { console.error("epic logout:", e); }
  epicGames = [];
  d.epicGrid.innerHTML = "";
  await loadEpic();
}

/** Kopiuje komendę instalacji legendary do schowka. */
async function epicCopyCommand() {
  const cmd = "winget install --id derrod.legendary";
  try {
    await navigator.clipboard.writeText(cmd);
    const label = d.epicCopyBtn.textContent;
    d.epicCopyBtn.textContent = t("amazon.copied");
    setTimeout(() => { d.epicCopyBtn.textContent = label; }, 1500);
  } catch (e) { console.error("clipboard:", e); }
}

/** Instaluje legendary przez winget (bez terminala) i odświeża status integracji. */
async function epicInstallTool() {
  const btn = d.epicInstallToolBtn;
  const label = btn.textContent;
  btn.disabled = true;
  btn.textContent = t("epic.installingTool");
  hide(d.epicSetupError);
  d.epicSetupError.textContent = "";
  d.epicSetupProgress.textContent = t("epic.toolProgress");
  show(d.epicSetupProgress);
  try {
    await invoke("epic_install_legendary");
  } catch (e) {
    d.epicSetupError.textContent = `${t("error.prefix")}: ${String(e)}`;
    show(d.epicSetupError);
  } finally {
    btn.disabled = false;
    btn.textContent = label;
    hide(d.epicSetupProgress);
  }
  await loadEpic();
}

/** Wybór folderu bazowego instalacji dla gry Epic. */
async function epicPickDir(id) {
  let dir = null;
  try { dir = await invoke("pick_epic_dir", { gameId: id }); }
  catch (e) { console.error("pick epic dir:", e); return; }
  if (!dir) return;
  const g = epicGames.find((x) => x.id === id);
  if (g) g.chosenDir = dir;
  renderEpicGames();
}

/** Pobiera i instaluje grę Epic (przez legendary); postęp przychodzi zdarzeniami. */
async function epicInstall(id) {
  const running = epicActive.get(id);
  if (running && running.ok == null) return; // już się instaluje
  const g = epicGames.find((x) => x.id === id);
  if (!g) return;
  if (!g.chosenDir) {
    await epicPickDir(id);
    if (!g.chosenDir) return; // anulowano wybór folderu
  }
  epicActive.set(id, { title: g.title, percent: null, line: t("amazon.starting"), ok: null, error: "" });
  renderStoreDownloads();
  try {
    await invoke("epic_install", { gameId: id });
    // Postęp jest w zakładce Pobierania — od razu ją pokazujemy (jak przy GOG).
    switchView("downloads");
  } catch (e) {
    epicActive.set(id, { title: g.title, percent: null, line: "", ok: false, error: String(e) });
    renderStoreDownloads();
  }
}

/** Uruchamia zainstalowaną grę Epic. */
async function epicLaunch(id) {
  const g = epicGames.find((x) => x.id === id);
  try {
    await invoke("epic_launch", { gameId: id });
    epicActive.delete(id);
    renderStoreDownloads();
  } catch (e) {
    epicActive.set(id, { title: g ? g.title : id, percent: null, line: "", ok: false, error: String(e) });
    renderStoreDownloads();
  }
}

/** Obsługa kliknięć na kafelkach Epic (delegacja — kafelki są przerysowywane). */
async function onEpicGridClick(e) {
  const launch = e.target.closest("[data-epic-launch]");
  if (launch) { await epicLaunch(launch.getAttribute("data-epic-launch")); return; }
  const install = e.target.closest("[data-epic-install]");
  if (install) { await epicInstall(install.getAttribute("data-epic-install")); return; }
  const dir = e.target.closest("[data-epic-dir]");
  if (dir) { await epicPickDir(dir.getAttribute("data-epic-dir")); return; }
  const open = e.target.closest("[data-epic-open]");
  if (open) openFolder(open.getAttribute("data-epic-open"));
}

/* ---------------------------- folder zapisu ---------------------------- */

async function refreshDownloadDir() {
  try { const dir = await invoke("get_download_dir"); setDirLabel(dir); } catch (e) {}
}

/** Aktualizuje znaczniki ścieżki (w widoku Pobierań i w Ustawieniach). */
function setDirLabel(dir) {
  currentDir = dir || "";
  const value = dir || "—";
  if (d.dlDirLabel) { d.dlDirLabel.textContent = t("downloads.folderPrefix") + " " + value; d.dlDirLabel.title = dir || ""; }
  if (d.setDirLabel) { d.setDirLabel.textContent = value; d.setDirLabel.title = dir || ""; }
}

/** Otwiera okno wyboru folderu; zwraca wybraną ścieżkę lub `null` (anulowano). */
async function chooseDownloadDir() {
  try {
    const dir = await invoke("pick_download_dir");
    if (dir) { setDirLabel(dir); return dir; }
    return null;
  } catch (e) { console.error("pick dir:", e); return null; }
}

/** Nowe pobranie: jeśli folder nie jest jeszcze ustawiony, najpierw o niego pytamy. */
async function startDownload(gid, iid, btn) {
  // Każda gra ma własny folder — pytamy o niego przy pierwszym pobraniu.
  if (!(await ensureGameDir(gid))) return; // anulowano wybór folderu
  if (btn) { btn.disabled = true; btn.textContent = t("dl.prepare"); }
  try {
    await invoke("start_download", { gameId: gid, installerId: iid });
    if (d.modal.open) closeModal();
    await loadDownloads();
    switchView("downloads");
  } catch (e) {
    console.error("download:", e);
    if (String(e).toLowerCase().includes("auth")) { loggedIn = false; renderAuth(); }
    else if (d.modal.open) { d.modalBody.innerHTML = `<p class="error">${t("error.prefix")}: ${esc(String(e))}</p>`; }
  } finally {
    if (btn) { btn.disabled = false; btn.textContent = t("modal.download"); }
  }
}

/** Wznowienie istniejącego pobrania (bez pytania o folder). */
async function resumeDl(gid, iid) {
  try {
    await invoke("start_download", { gameId: gid, installerId: iid });
    await loadDownloads();
    switchView("downloads");
  } catch (e) {
    console.error(e);
    if (String(e).toLowerCase().includes("auth")) { loggedIn = false; renderAuth(); }
  }
}

async function pauseDl(id) { try { await invoke("pause_download", { downloadId: id }); } catch (e) {} await loadDownloads(); }
async function cancelDl(id) { try { await invoke("cancel_download", { downloadId: id }); } catch (e) {} await loadDownloads(); }

async function loadDownloads() {
  let rows;
  try { rows = await invoke("get_downloads"); } catch (e) { rows = []; }
  dlRows.clear();
  for (const r of rows) dlRows.set(r.id, r);
  renderDownloads();
  // Stany na kafelkach biblioteki i przy instalatorach zależą od listy pobrań.
  if (gamesCache.length) renderGames();
}

function renderDownloads() {
  d.dlList.innerHTML = "";
  const rows = [...dlRows.values()].sort((a, b) => b.id - a.id);
  // Pobrania sklepów liczą się do tej samej listy i plakietki w menu.
  const storeRunning = [...amazonActive.values()].filter((a) => a.ok == null).length +
    [...epicActive.values()].filter((a) => a.ok == null).length;
  if (!rows.length && !amazonActive.size && !epicActive.size) {
    d.dlList.innerHTML = `<p class="empty-state">${t("downloads.empty")}</p>`;
    hide(d.dlBadge);
    return;
  }
  const active = rows.filter((x) => x.status === "downloading").length + storeRunning;
  d.dlBadge.textContent = active > 0 ? String(active) : "\u2713";
  show(d.dlBadge);
  for (const x of rows) {
    const el = document.createElement("div");
    el.className = "download-item"; el.dataset.dlId = x.id;
    paintDl(el, x);
    d.dlList.appendChild(el);
  }
}

/** Procent postępu (0–100) dla wiersza pobierania. */
function dlPct(row) {
  let pct = row.progress != null ? row.progress : (row.totalBytes > 0 ? (row.downloadedBytes / row.totalBytes * 100) : 0);
  if (!isFinite(pct) || pct < 0) pct = row.status === "completed" ? 100 : 0;
  return pct;
}

/** Klasa CSS opisująca stan wiersza. */
function dlClass(status) {
  return ["completed", "failed", "paused", "cancelled", "missing"].includes(status) ? status : "active";
}

/** Status do wyświetlenia: ukończone pobranie bez pliku na dysku to „missing”. */
function displayStatus(row) {
  return row.status === "completed" && !row.fileExists ? "missing" : row.status;
}

/**
 * Pełny render kafelka pobierania (pasek + liczby + przyciski akcji).
 * Wywoływany przy budowie listy oraz gdy zmieni się status.
 */
function paintDl(el, row) {
  const pct = Math.min(dlPct(row), 100);
  const status = displayStatus(row);
  const cls = dlClass(status);
  const busy = row.status === "downloading" || row.status === "pending";
  el.dataset.status = status;
  el.innerHTML = `
    <div class="dl-head">
      <div class="dl-title">${esc(row.gameTitle)}</div>
      <div class="dl-status ${cls}">${statusLabel(status)}</div>
    </div>
    <div class="dl-filename">${esc(row.filename)}</div>
    <div class="dl-path" title="${esc(row.targetPath)}">${esc(row.targetPath)}</div>
    ${status === "missing" ? `<div class="dl-missing">${t("dl.missingHint")}</div>` : ""}
    <div class="dl-progress-track"><div class="dl-progress-fill ${cls}" style="width:${pct}%"></div></div>
    <div class="dl-info">
      <span>${fmtSize(row.downloadedBytes)} / ${fmtSize(row.totalBytes)}</span>
      <span class="dl-pct">${pct.toFixed(1)}%</span>
    </div>
    ${row.error ? `<div class="dl-error">${esc(row.error)}</div>` : ""}
    <div class="dl-actions">
      ${row.status === "completed" ? `<button class="btn btn-small" onclick="openDlFolder(${row.id})">${t("folder.open")}</button>` : ""}
      ${busy ? `<button class="btn btn-small" onclick="pauseDl(${row.id})">${t("dl.stop")}</button>` : ""}
      ${(row.status === "paused" || row.status === "failed") ? `<button class="btn btn-small btn-primary" onclick="resumeDl(${row.gameId},'${esc(row.installerId)}')">${t("dl.resume")}</button>` : ""}
      ${(busy || row.status === "paused" || row.status === "failed") ? `<button class="btn btn-small" onclick="cancelDl(${row.id})">${t("dl.cancel")}</button>` : ""}
    </div>`;
}

/**
 * Lekka aktualizacja postępu — zmienia tylko szerokość paska i liczby,
 * nie dotykając przycisków (dzięki temu klikanie jest stabilne i pasek nie miga).
 */
function patchDlProgress(el, row) {
  const pct = Math.min(dlPct(row), 100);
  const fill = el.querySelector(".dl-progress-fill");
  if (fill) fill.style.width = pct + "%";
  const info = el.querySelector(".dl-info");
  if (info) {
    info.innerHTML = `<span>${fmtSize(row.downloadedBytes)} / ${fmtSize(row.totalBytes)}</span>` +
      `<span class="dl-pct">${pct.toFixed(1)}%</span>`;
  }
}

function esc(s) { return !s ? "" : String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#039;" })[c]); }
function fmtSize(b) { b = parseInt(b) || 0; if (b < 1024) return b + " B"; if (b < 1048576) return (b/1024).toFixed(1) + " KiB"; if (b < 1073741824) return (b/1048576).toFixed(1) + " MiB"; return (b/1073741824).toFixed(2) + " GiB"; }

window.pauseDl = pauseDl;
window.cancelDl = cancelDl;
window.resumeDl = resumeDl;
window.openDlFolder = openDlFolder;
window.closeModal = closeModal;
document.addEventListener("DOMContentLoaded", init);