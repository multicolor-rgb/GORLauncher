// goglite — prosty mechanizm wielojęzyczności (PL / EN)
// Słowniki + tłumaczenie atrybutów [data-i18n] oraz tekstów generowanych w JS.

const DICT = {
  pl: {
    "app.title": "goglite — instalatory offline GOG",
    "app.tagline": "Menedżer instalatorów offline",
    // Etykiety HUD (numerowane sekcje)
    "hud.library": "Biblioteka",
    "hud.catalog": "Katalog",
    "hud.downloads": "Pobierania",
    "hud.settings": "Konfiguracja",
    "hud.auth": "Dostęp",
    "hud.game": "Gra",
    // Nawigacja
    "nav.library": "Biblioteka",
    "nav.downloads": "Pobierania",
    "nav.settings": "Ustawienia",
    // Akcje
    "action.sync": "Synchronizuj",
    "action.logout": "Wyloguj",
    "topbar.search": "Szukaj w bibliotece…",
    // Powitanie
    "welcome.title": "Witaj w goglite",
    "welcome.desc1": "Lekki menedżer pobierania instalatorów offline z GOG.com.",
    "welcome.desc2": "Zaloguj się przez okno GOG, a następnie zsynchronizuj bibliotekę.",
    "welcome.login": "Zaloguj przez GOG",
    // Synchronizacja / biblioteka
    "sync.syncing": "Synchronizowanie biblioteki…",
    "library.title": "Moja biblioteka.",
    "library.sub": "Instalatory offline z GOG.com. Wybierz grę, aby zobaczyć pliki.",
    "library.countLabel": "pozycji",
    "library.emptyPrefix": "Brak gier. Kliknij ",
    "library.emptyAction": "Synchronizuj",
    "library.emptySuffix": ".",
    "library.loadError": "Błąd wczytywania biblioteki",
    "library.noResults": "Brak wyników.",
    // Pobierania
    "downloads.title": "Pobierania",
    "downloads.sub": "Kolejka i historia pobrań instalatorów.",
    "downloads.empty": "Brak pobierań.",
    "downloads.folderPrefix": "Domyślny folder:",
    "downloads.changeInSettings": "Zmień w ustawieniach",
    // Ustawienia
    "settings.title": "Ustawienia",
    "settings.sub": "Język, wygląd i miejsce zapisu instalatorów.",
    "settings.language": "Język",
    "settings.language.desc": "Wybierz język interfejsu aplikacji.",
    "settings.theme": "Motyw",
    "settings.theme.desc": "Wybierz jasny lub ciemny wygląd aplikacji.",
    "settings.theme.light": "Jasny",
    "settings.theme.dark": "Ciemny",
    "settings.dir": "Domyślny folder zapisu",
    "settings.dir.desc": "Używany, gdy gra nie ma własnego folderu pobierania.",
    "settings.dir.change": "Zmień folder…",
    "settings.about": "O aplikacji",
    "settings.about.desc": "Lekki menedżer instalatorów offline GOG.",
    // Uwierzytelnianie
    "auth.online": "Online",
    "auth.offline": "Offline",
    "auth.opening": "Otwieranie okna logowania…",
    "auth.loginError": "Błąd logowania",
    "auth.unknown": "nieznany błąd",
    // Statusy pobierań
    "status.pending": "Oczekuje",
    "status.downloading": "Pobieranie",
    "status.paused": "Wstrzymane",
    "status.completed": "Ukończone",
    "status.failed": "Błąd",
    "status.cancelled": "Anulowane",
    "status.missing": "Plik usunięty",
    // Stany plików gier (biblioteka + instalatory)
    "game.folder": "Folder gry",
    "game.folderHint": "domyślny",
    "game.folder.change": "Zmień folder…",
    "folder.open": "Otwórz folder",
    "game.state.downloaded": "Pobrane",
    "game.state.downloading": "W trakcie pobierania",
    "game.state.missing": "Brak pliku",
    // Modal
    "modal.loading": "Ładowanie…",
    "modal.noInstallers": "Brak instalatorów dla tej gry.",
    "modal.download": "Pobierz",
    "modal.genre": "Gatunek",
    "modal.developer": "Producent",
    "modal.publisher": "Wydawca",
    "modal.releaseDate": "Premiera",
    "modal.installers": "Instalatory",
    "modal.tabDescription": "Opis",
    "modal.tabInstallers": "Instalatory",
    "modal.screenshots": "Zrzuty ekranu",
    "modal.noDescription": "Brak opisu dla tej gry.",
    // Pobieranie (akcje)
    "dl.prepare": "Przygotowywanie…",
    "dl.stop": "Zatrzymaj",
    "dl.resume": "Wznów",
    "dl.cancel": "Anuluj",
    "dl.missingHint": "Pliku nie ma już w wybranym folderze — pobierz go ponownie.",
    // Amazon Games (Prime Gaming)
    "nav.amazon": "Prime Gaming",
    "hud.amazon": "Amazon Games",
    "amazon.title": "Prime Gaming",
    "amazon.sub": "Gry z klienta Amazon Games. Zaloguj się kontem Amazon, aby je zobaczyć, pobrać i uruchomić.",
    "amazon.missing": "Do obsługi Amazon Games potrzebne jest narzędzie nile.",
    "amazon.installHint": "Możemy zainstalować je za Ciebie przez winget — bez terminala i bez uprawnień administratora.",
    "amazon.installNile": "Zainstaluj nile",
    "amazon.installingNile": "Instalowanie nile…",
    "amazon.nileProgress": "Uruchamiam winget… pobranie i rozpakowanie może potrwać do kilku minut.",
    "amazon.copyCommand": "Kopiuj komendę",
    "amazon.copied": "Skopiowano",
    "amazon.recheck": "Sprawdź ponownie",
    "amazon.login": "Zaloguj do Amazon",
    "amazon.loginOpening": "Logowanie…",
    "amazon.logout": "Wyloguj",
    "amazon.refresh": "Odśwież bibliotekę",
    "amazon.syncing": "Synchronizowanie…",
    "amazon.account": "Konto Amazon",
    "amazon.notLoggedIn": "Nie zalogowano",
    "amazon.loginHint": "Zaloguj się kontem Amazon, aby zobaczyć gry z klienta Amazon Games.",
    "amazon.loginSteps": "Zaloguj się w otwartej przeglądarce. Gdy Amazon przekieruje Cię na stronę amazon.com, skopiuj jej pełny adres z paska przeglądarki i wklej go poniżej.",
    "amazon.loginPlaceholder": "https://www.amazon.com/?openid.oa2.authorization_code=…",
    "amazon.loginFinish": "Zakończ logowanie",
    "amazon.loginFinishing": "Logowanie…",
    "amazon.loginCancel": "Anuluj",
    "amazon.loginNoUrl": "Wklej adres, na który przekierowała przeglądarka po zalogowaniu.",
    "amazon.empty": "Brak gier w bibliotece Amazon. Odśwież bibliotekę po przypisaniu gier w Prime Gaming.",
    "amazon.install": "Pobierz",
    "amazon.play": "Uruchom",
    "amazon.folder": "Folder…",
    "amazon.installed": "Zainstalowana",
    "amazon.starting": "Przygotowywanie…",
    "amazon.done": "Gotowe",
    "amazon.failed": "Błąd",
    "error.prefix": "Błąd",
    // Filtr platformy (co można zainstalować na Windows/Linux/macOS)
    "filter.all": "Wszystkie",
    "filter.windows": "Windows",
    "filter.linux": "Linux",
    "filter.macos": "macOS",
    "modal.noInstallersForPlatform": "Brak instalatorów dla wybranej platformy.",
  },
  en: {
    "app.title": "goglite — GOG offline installers",
    "app.tagline": "Offline installer manager",
    "hud.library": "Library",
    "hud.catalog": "Catalog",
    "hud.downloads": "Downloads",
    "hud.settings": "Configuration",
    "hud.auth": "Access",
    "hud.game": "Game",
    "nav.library": "Library",
    "nav.downloads": "Downloads",
    "nav.settings": "Settings",
    "action.sync": "Sync",
    "action.logout": "Log out",
    "topbar.search": "Search library…",
    "welcome.title": "Welcome to goglite",
    "welcome.desc1": "A lightweight manager for downloading GOG.com offline installers.",
    "welcome.desc2": "Sign in through the GOG window, then sync your library.",
    "welcome.login": "Sign in with GOG",
    "sync.syncing": "Syncing library…",
    "library.title": "My library.",
    "library.sub": "Offline installers from GOG.com. Pick a game to see its files.",
    "library.countLabel": "items",
    "library.emptyPrefix": "No games. Click ",
    "library.emptyAction": "Sync",
    "library.emptySuffix": ".",
    "library.loadError": "Failed to load library",
    "library.noResults": "No results.",
    "downloads.title": "Downloads",
    "downloads.sub": "Installer download queue and history.",
    "downloads.empty": "No downloads.",
    "downloads.folderPrefix": "Default folder:",
    "downloads.changeInSettings": "Change in settings",
    "settings.title": "Settings",
    "settings.sub": "Language, appearance and where installers are saved.",
    "settings.language": "Language",
    "settings.language.desc": "Choose the application interface language.",
    "settings.theme": "Theme",
    "settings.theme.desc": "Choose a light or dark appearance.",
    "settings.theme.light": "Light",
    "settings.theme.dark": "Dark",
    "settings.dir": "Default download folder",
    "settings.dir.desc": "Used when a game has no folder of its own.",
    "settings.dir.change": "Change folder…",
    "settings.about": "About",
    "settings.about.desc": "A lightweight manager for GOG offline installers.",
    "auth.online": "Online",
    "auth.offline": "Offline",
    "auth.opening": "Opening login window…",
    "auth.loginError": "Login error",
    "auth.unknown": "unknown error",
    "status.pending": "Pending",
    "status.downloading": "Downloading",
    "status.paused": "Paused",
    "status.completed": "Completed",
    "status.failed": "Failed",
    "status.cancelled": "Cancelled",
    "status.missing": "File removed",
    "game.folder": "Game folder",
    "game.folderHint": "default",
    "game.folder.change": "Change folder…",
    "folder.open": "Open folder",
    "game.state.downloaded": "Downloaded",
    "game.state.downloading": "Downloading",
    "game.state.missing": "File missing",
    "modal.loading": "Loading…",
    "modal.noInstallers": "No installers for this game.",
    "modal.download": "Download",
    "modal.genre": "Genre",
    "modal.developer": "Developer",
    "modal.publisher": "Publisher",
    "modal.releaseDate": "Release date",
    "modal.installers": "Installers",
    "modal.tabDescription": "Description",
    "modal.tabInstallers": "Installers",
    "modal.screenshots": "Screenshots",
    "modal.noDescription": "No description for this game.",
    "dl.prepare": "Preparing…",
    "dl.stop": "Stop",
    "dl.resume": "Resume",
    "dl.cancel": "Cancel",
    "dl.missingHint": "The file is no longer in the selected folder — download it again.",
    "nav.amazon": "Prime Gaming",
    "hud.amazon": "Amazon Games",
    "amazon.title": "Prime Gaming",
    "amazon.sub": "Games from the Amazon Games client. Sign in with your Amazon account to see, download and launch them.",
    "amazon.missing": "Amazon Games support needs the nile tool.",
    "amazon.installHint": "We can install it for you via winget — no terminal, no administrator rights.",
    "amazon.installNile": "Install nile",
    "amazon.installingNile": "Installing nile…",
    "amazon.nileProgress": "Starting winget… downloading and unpacking can take a few minutes.",
    "amazon.copyCommand": "Copy command",
    "amazon.copied": "Copied",
    "amazon.recheck": "Check again",
    "amazon.login": "Sign in to Amazon",
    "amazon.loginOpening": "Signing in…",
    "amazon.logout": "Sign out",
    "amazon.refresh": "Refresh library",
    "amazon.syncing": "Syncing…",
    "amazon.account": "Amazon account",
    "amazon.notLoggedIn": "Not signed in",
    "amazon.loginHint": "Sign in with your Amazon account to see games from the Amazon Games client.",
    "amazon.loginSteps": "Sign in in the browser window that just opened. When Amazon redirects you to amazon.com, copy the full address from the address bar and paste it below.",
    "amazon.loginPlaceholder": "https://www.amazon.com/?openid.oa2.authorization_code=…",
    "amazon.loginFinish": "Finish sign-in",
    "amazon.loginFinishing": "Signing in…",
    "amazon.loginCancel": "Cancel",
    "amazon.loginNoUrl": "Paste the address the browser was redirected to after sign-in.",
    "amazon.empty": "No games in your Amazon library. Refresh after claiming games on Prime Gaming.",
    "amazon.install": "Download",
    "amazon.play": "Launch",
    "amazon.folder": "Folder…",
    "amazon.installed": "Installed",
    "amazon.starting": "Preparing…",
    "amazon.done": "Done",
    "amazon.failed": "Failed",
    "error.prefix": "Error",
    "filter.all": "All",
    "filter.windows": "Windows",
    "filter.linux": "Linux",
    "filter.macos": "macOS",
    "modal.noInstallersForPlatform": "No installers for the selected platform.",
  },
};

export const LANGUAGES = ["pl", "en"];
let current = "pl";

/** Normalizuje dowolny kod języka (np. `pl-PL`, `en-US`) do `"pl"`/`"en"` lub `null`. */
export function normalizeLang(raw) {
  if (!raw) return null;
  const l = String(raw).toLowerCase();
  if (l.startsWith("pl")) return "pl";
  if (l.startsWith("en")) return "en";
  return null;
}

/** Zwraca bieżący kod języka. */
export function currentLang() {
  return current;
}

/** Tłumaczy klucz; gdy brak, zwraca `fallback` lub sam klucz. */
export function t(key, fallback) {
  const dict = DICT[current] || DICT.pl;
  return dict[key] != null ? dict[key] : (fallback != null ? fallback : key);
}

/** Podmienia teksty statyczne (`[data-i18n]`), placeholdery i title oraz tytuł okna i `lang`. */
export function applyTranslations(root = document) {
  root.querySelectorAll("[data-i18n]").forEach((el) => {
    el.textContent = t(el.getAttribute("data-i18n"));
  });
  root.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
    el.setAttribute("placeholder", t(el.getAttribute("data-i18n-placeholder")));
  });
  root.querySelectorAll("[data-i18n-title]").forEach((el) => {
    el.setAttribute("title", t(el.getAttribute("data-i18n-title")));
  });
  document.title = t("app.title");
  document.documentElement.lang = current;
  syncLangSwitch();
}

/** Ustawia język (waliduje kod) i natychmiast stosuje tłumaczenia do DOM. */
export function setLanguage(lang) {
  const norm = normalizeLang(lang);
  current = norm || "pl";
  applyTranslations();
  return current;
}

/** Podświetla aktywny przycisk w przełączniku języka (jeśli istnieje). */
function syncLangSwitch() {
  document.querySelectorAll("#lang-switch [data-lang]").forEach((btn) => {
    btn.classList.toggle("active", btn.getAttribute("data-lang") === current);
  });
}
