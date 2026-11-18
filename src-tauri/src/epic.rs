// goglite — integracja z Epic Games
// Epic nie udostępnia publicznego API dla klienta gier, więc korzystamy z `legendary`
// (otwarty klient Epic Games Store, ten sam, którego używa Heroic):
//   legendary status --json               (stan konta)
//   legendary auth --code <kod>           (logowanie kodem ze strony logowania)
//   legendary auth --delete               (wylogowanie)
//   legendary list --json                 (biblioteka + metadane z okładkami)
//   legendary list-installed --json       (co jest zainstalowane i gdzie)
//   legendary install <app> --base-path <folder> -y --skip-sdl
//   legendary launch <app>
// Dzięki temu logowanie, pobieranie i uruchamianie obsługuje dojrzałe narzędzie,
// a goglite dokłada UI, foldery i stan instalacji.

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::cli;
use crate::commands::AppState;
use crate::db::Database;

/// Komunikat zwracany, gdy `legendary` nie jest zainstalowany.
const LEGENDARY_MISSING: &str =
    "Nie znaleziono narzędzia legendary. Zainstaluj je komendą: winget install --id derrod.legendary";

/// Strona logowania: po zalogowaniu wyświetla JSON z polem `authorizationCode`.
const EPIC_LOGIN_URL: &str = "https://legendary.gl/epiclogin";

/// Ile czekamy na odpowiedź `legendary` (status, lista zainstalowanych, logowanie).
const EPIC_READ_TIMEOUT: Duration = Duration::from_secs(120);
/// Pobranie całej biblioteki z metadanymi potrafi trwać minuty.
const EPIC_SYNC_TIMEOUT: Duration = Duration::from_secs(600);

/// Stan integracji widoczny w UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpicStatus {
    /// Czy `legendary` jest dostępny na tym komputerze.
    pub available: bool,
    /// Ścieżka do znalezionego pliku `legendary`.
    pub legendary_path: Option<String>,
    pub logged_in: bool,
    pub username: Option<String>,
}

/// Gra z biblioteki Epic (połączona z lokalnym stanem instalacji).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpicGame {
    /// Nazwa aplikacji w Epic (`app_name`) — używana przez `legendary`.
    pub id: String,
    pub title: String,
    /// Producent (Epic podaje go w metadanych katalogowych).
    pub developer: Option<String>,
    /// Adres okładki z `metadata.keyImages`.
    pub cover_url: Option<String>,
    /// Rzeczywisty folder zainstalowanej gry (z `list-installed`).
    pub install_dir: Option<String>,
    /// Folder bazowy wybrany przez użytkownika (zapamiętany dla kolejnych instalacji).
    pub chosen_dir: Option<String>,
    pub installed: bool,
    /// Czy folder instalacji nadal istnieje (gra mogła zostać usunięta z dysku).
    pub dir_exists: bool,
}

// —————————— URUCHAMIANIE LEGENDARY ——————————

/// Nazwa pliku wykonywalnego `legendary` na tym systemie.
fn legendary_exe() -> &'static str {
    if cfg!(windows) {
        "legendary.exe"
    } else {
        "legendary"
    }
}

/// Szuka `legendary` w `GOGLITE_LEGENDARY`, instalacji winget i w PATH.
fn find_legendary() -> Option<PathBuf> {
    cli::find_tool("GOGLITE_LEGENDARY", legendary_exe(), "derrod.legendary")
}

/// Uruchamia `legendary` i zwraca `stdout` (błąd procesu → komunikat z `stderr`).
fn run_legendary(args: &[&str], limit: Duration) -> Result<String, String> {
    let bin = find_legendary().ok_or_else(|| LEGENDARY_MISSING.to_string())?;
    let mut cmd = cli::tool_command(&bin);
    cmd.args(args);
    let out = cli::run_with_timeout(cmd, limit, |_| {})?;
    if out.timed_out {
        return Err(format!(
            "legendary {} nie odpowiedział w wyznaczonym czasie",
            args.join(" ")
        ));
    }
    if !out.success() {
        let err = out.stderr.trim().to_string();
        return Err(if err.is_empty() {
            format!("legendary {} zakończył się błędem", args.join(" "))
        } else {
            err
        });
    }
    Ok(out.stdout)
}

// —————————— STATUS / LOGOWANIE ——————————

/// Fragment `legendary status --json` — interesuje nas tylko konto.
#[derive(Deserialize)]
struct EpicAccount {
    #[serde(default)]
    account: Option<String>,
}

/// Odczytuje stan logowania z `legendary status --json`.
fn auth_status() -> (bool, Option<String>) {
    let Ok(raw) = run_legendary(&["status", "--json"], EPIC_READ_TIMEOUT) else {
        return (false, None);
    };
    match cli::extract_json::<EpicAccount>(&raw) {
        Some(st) => match st.account {
            Some(name) if !name.trim().is_empty() => (true, Some(name)),
            _ => (false, None),
        },
        None => (false, None),
    }
}

/// Odczytuje stan integracji (uruchamia `legendary` — tylko poza wątkiem UI).
fn status_snapshot() -> EpicStatus {
    let Some(bin) = find_legendary() else {
        return EpicStatus {
            available: false,
            legendary_path: None,
            logged_in: false,
            username: None,
        };
    };
    let (logged_in, username) = auth_status();
    EpicStatus {
        available: true,
        legendary_path: Some(bin.to_string_lossy().to_string()),
        logged_in,
        username,
    }
}

/// Zwraca informacje o dostępności `legendary` i zalogowanym koncie Epic.
/// Komenda jest `async`, bo start Pythona (legendary) blokowałby wątek UI.
#[tauri::command]
pub async fn epic_status() -> EpicStatus {
    tauri::async_runtime::spawn_blocking(status_snapshot)
        .await
        .unwrap_or_else(|_| EpicStatus {
            available: false,
            legendary_path: None,
            logged_in: false,
            username: None,
        })
}

/// Otwiera stronę logowania Epic.
///
/// `legendary auth` bez argumentów pyta o kod w terminalu (`input()`), co bez konsoli kończy
/// się `EOFError`. Dlatego logowanie robimy w dwóch krokach: otwieramy stałą stronę
/// `legendary.gl/epiclogin`, a użytkownik wkleja kod w UI (`epic_login_complete`).
#[tauri::command]
pub async fn epic_login(app: AppHandle) -> Result<(), String> {
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(EPIC_LOGIN_URL, None::<&str>)
        .map_err(|e| format!("Nie udało się otworzyć przeglądarki: {e}"))
}

/// Wyciąga `authorizationCode` z tego, co użytkownik wklei ze strony logowania:
/// pełnego JSON-a, adresu z parametrem albo samego kodu.
fn extract_epic_code(input: &str) -> Option<String> {
    let text = input.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        for key in ["authorizationCode", "code"] {
            if let Some(code) = value[key].as_str().map(str::trim).filter(|c| !c.is_empty()) {
                return Some(code.to_string());
            }
        }
    }
    if let Some(pos) = text.find("authorizationCode") {
        let rest = &text[pos + "authorizationCode".len()..];
        let rest = rest.trim_start_matches(['"', '\'', ' ', ':', '=']);
        let code: String = rest
            .chars()
            .take_while(|c| *c != '"' && *c != '&' && !c.is_whitespace())
            .collect();
        if !code.is_empty() {
            return Some(code);
        }
    }
    // Sam kod (np. 32 znaki hex) — bez adresów, nawiasów i spacji.
    let bare = text.trim_matches('"').trim();
    let looks_like_code = bare.len() >= 8
        && !bare.contains([':', '/', '{', '}', ' '])
        && !bare.contains('&')
        && !bare.contains('\n');
    if looks_like_code {
        return Some(bare.to_string());
    }
    None
}

/// Dokańcza logowanie kodem ze strony `legendary.gl/epiclogin`.
#[tauri::command]
pub async fn epic_login_complete(code: String) -> Result<EpicStatus, String> {
    let code = extract_epic_code(&code).ok_or_else(|| {
        "Nie znaleziono kodu logowania. Skopiuj ze strony logowania cały JSON (pole authorizationCode) albo sam kod."
            .to_string()
    })?;
    tauri::async_runtime::spawn_blocking(move || -> Result<EpicStatus, String> {
        let result = run_legendary(&["auth", "--code", &code], EPIC_SYNC_TIMEOUT);
        // `auth` mogło się udać, a błąd dotyczyć tylko sprawdzenia stanu konta.
        let status = status_snapshot();
        if status.logged_in {
            return Ok(status);
        }
        result?;
        Err("Epic nie potwierdził logowania. Skopiuj nowy kod ze strony logowania i spróbuj ponownie."
            .to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Wylogowanie konta Epic (`legendary auth --delete`).
#[tauri::command]
pub async fn epic_logout() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        run_legendary(&["auth", "--delete"], EPIC_READ_TIMEOUT).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

// —————————— INSTALACJA LEGENDARY (winget) ——————————

/// Instaluje `legendary` przez winget — bez terminala i bez uprawnień administratora.
fn install_legendary_via_winget(app: &AppHandle) -> Result<(), String> {
    if find_legendary().is_some() {
        return Ok(());
    }
    let out = cli::winget_install(app, "epic:tool-progress", "derrod.legendary")?;
    // Winget potrafi zwrócić błąd „już zainstalowano” — rozstrzyga obecność pliku.
    if find_legendary().is_some() {
        return Ok(());
    }
    if out.timed_out {
        return Err(
            "Instalacja legendary przez winget trwała zbyt długo i została przerwana. Spróbuj ponownie albo zainstaluj ręcznie: winget install --id derrod.legendary"
                .to_string(),
        );
    }
    if out.success() {
        return Err(
            "legendary został zainstalowany, ale nie znaleziono pliku legendary.exe — uruchom goglite ponownie."
                .to_string(),
        );
    }
    let raw = format!("{}{}", out.stdout, out.stderr);
    let msg = raw
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| l.len() > 3 && !l.contains('█'))
        .unwrap_or("")
        .to_string();
    Err(if msg.is_empty() {
        "winget nie zainstalował legendary.".to_string()
    } else {
        msg
    })
}

/// Instaluje `legendary` przyciskiem w aplikacji i zwraca nowy stan integracji.
#[tauri::command]
pub async fn epic_install_legendary(app: AppHandle) -> Result<EpicStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_legendary_via_winget(&app)?;
        let status = status_snapshot();
        if !status.available {
            return Err("Nie znaleziono legendary po instalacji.".to_string());
        }
        Ok(status)
    })
    .await
    .map_err(|e| e.to_string())?
}

// —————————— BIBLIOTEKA ——————————

/// Typy okładek z `metadata.keyImages`, w kolejności preferencji (najpierw poziome).
const COVER_TYPES: [&str; 6] = [
    "DieselGameBox",
    "OfferImageWide",
    "DieselGameBoxWide",
    "DieselGameBoxTall",
    "OfferImageTall",
    "Thumbnail",
];

/// Wybiera najlepszą okładkę z metadanych katalogowych Epic.
fn cover_url(metadata: &serde_json::Value) -> Option<String> {
    let images = metadata["keyImages"].as_array()?;
    for wanted in COVER_TYPES {
        if let Some(url) = images
            .iter()
            .find(|img| img["type"].as_str() == Some(wanted))
            .and_then(|img| img["url"].as_str())
            .filter(|u| u.starts_with("http"))
        {
            return Some(url.to_string());
        }
    }
    images
        .first()
        .and_then(|img| img["url"].as_str())
        .filter(|u| u.starts_with("http"))
        .map(str::to_string)
}

/// Czy gra ma build na tę platformę (na Windowsie pomijamy tytuły tylko-macOS).
fn has_current_platform(item: &serde_json::Value) -> bool {
    // Na innych systemach nie filtrujemy — legendary domyślnie pobiera build Windows.
    if !cfg!(windows) {
        return true;
    }
    let Some(infos) = item["asset_infos"].as_object() else {
        return true;
    };
    if infos.is_empty() {
        return true;
    }
    infos.keys().any(|k| k.to_lowercase().contains("windows"))
}

/// Mapuje `list-installed --json` na `app_name -> folder instalacji`.
fn parse_installed(raw: &str) -> HashMap<String, Option<String>> {
    let Some(items) = cli::extract_json::<Vec<serde_json::Value>>(raw) else {
        return HashMap::new();
    };
    let mut map = HashMap::new();
    for item in items {
        let Some(app) = item["app_name"].as_str().filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        let path = item["install_path"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string);
        map.insert(app.to_string(), path);
    }
    map
}

/// Parsuje `legendary list --json` (plus `list-installed`) i dokłada lokalny stan instalacji.
fn parse_library(raw: &str, installed_raw: &str, db: &Database) -> Result<Vec<EpicGame>, String> {
    let items = cli::extract_json::<Vec<serde_json::Value>>(raw)
        .ok_or_else(|| "Nie udało się odczytać biblioteki legendary.".to_string())?;
    let installed = parse_installed(installed_raw);
    let states = db.epic_states().map_err(|e| e.to_string())?;
    let mut games = Vec::new();
    for item in items {
        let Some(id) = item["app_name"].as_str().filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        let metadata = &item["metadata"];
        // DLC i dodatki mają odwołanie do gry głównej — na liście pokazujemy tylko gry.
        if metadata
            .get("mainGameItem")
            .map(|v| !v.is_null())
            .unwrap_or(false)
        {
            continue;
        }
        if !has_current_platform(&item) {
            continue;
        }
        let title = item["app_title"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(id);
        let developer = metadata["developer"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string);
        let install_dir = installed.get(id).cloned().flatten();
        let dir_exists = install_dir
            .as_deref()
            .map(|d| Path::new(d).is_dir())
            .unwrap_or(false);
        let chosen_dir = states.get(id).and_then(|(dir, _)| dir.clone());
        games.push(EpicGame {
            id: id.to_string(),
            title: title.to_string(),
            developer,
            cover_url: cover_url(metadata),
            installed: install_dir.is_some() && dir_exists,
            install_dir,
            chosen_dir,
            dir_exists,
        });
    }
    games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(games)
}

/// Biblioteka Epic (`legendary list --json` + `list-installed --json`).
#[tauri::command]
pub async fn epic_library(state: State<'_, AppState>) -> Result<Vec<EpicGame>, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let raw = run_legendary(&["list", "--json"], EPIC_SYNC_TIMEOUT)?;
        // Brak `list-installed` nie może przewrócić całej biblioteki.
        let installed = run_legendary(&["list-installed", "--json"], EPIC_READ_TIMEOUT).unwrap_or_default();
        parse_library(&raw, &installed, &db)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Wymusza ponowne pobranie metadanych biblioteki (`legendary list --json --force-refresh`).
#[tauri::command]
pub async fn epic_sync() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        run_legendary(&["list", "--json", "--force-refresh"], EPIC_SYNC_TIMEOUT).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

// —————————— FOLDER GRY ——————————

/// Wybór folderu bazowego instalacji dla gry Epic; zapisuje go w bazie.
#[tauri::command]
pub async fn pick_epic_dir(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let start = state
        .db
        .get_epic_install_dir(&game_id)
        .ok()
        .flatten()
        .map(PathBuf::from)
        .unwrap_or_else(|| state.download_dir.lock().unwrap().clone());
    let picked = app
        .dialog()
        .file()
        .set_directory(&start)
        .blocking_pick_folder();
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let dir = path.to_string_lossy().to_string();
    state
        .db
        .set_epic_install_dir(&game_id, Some(&dir))
        .map_err(|e| e.to_string())?;
    Ok(Some(dir))
}

// —————————— POBIERANIE / URUCHAMIANIE ——————————

fn emit_done(app: &AppHandle, game_id: &str, ok: bool, error: &str) {
    let _ = app.emit(
        "epic:done",
        serde_json::json!({ "gameId": game_id, "ok": ok, "error": error }),
    );
}

/// Pobiera i instaluje grę przez `legendary install <app> --base-path <folder>`.
/// Postęp leci zdarzeniami `epic:progress`, zakończenie — `epic:done`.
#[tauri::command]
pub fn epic_install(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: String,
) -> Result<(), String> {
    let bin = find_legendary().ok_or_else(|| LEGENDARY_MISSING.to_string())?;
    let dir = state
        .db
        .get_epic_install_dir(&game_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Najpierw wybierz folder instalacji dla tej gry.".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db = state.db.clone();
    let _ = db.set_epic_installed(&game_id, false);

    std::thread::spawn(move || {
        let mut cmd = cli::tool_command(&bin);
        let mut child = match cmd
            .args([
                "install",
                game_id.as_str(),
                "--base-path",
                dir.as_str(),
                "-y",
                "--skip-sdl",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                emit_done(
                    &app,
                    &game_id,
                    false,
                    &format!("Nie udało się uruchomić legendary: {e}"),
                );
                return;
            }
        };

        // legendary loguje na stdout — to stamtąd bierzemy postęp.
        // stderr musi być czytane, żeby pełny bufor nie zablokował procesu.
        if let Some(stderr) = child.stderr.take() {
            std::thread::spawn(move || {
                for _ in BufReader::new(stderr).lines() {}
            });
        }

        let mut last_line = String::new();
        if let Some(stdout) = child.stdout.take() {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let percent = cli::parse_percent(&line);
                let Some(text) = cli::progress_text(&line, percent) else {
                    continue;
                };
                last_line = text.clone();
                let _ = app.emit(
                    "epic:progress",
                    serde_json::json!({ "gameId": game_id, "line": text, "percent": percent }),
                );
            }
        }

        let ok = matches!(child.wait(), Ok(status) if status.success());
        if ok {
            let _ = db.set_epic_installed(&game_id, true);
        }
        let error = if ok { String::new() } else { last_line };
        emit_done(&app, &game_id, ok, &error);
    });
    Ok(())
}

/// Uruchamia zainstalowaną grę (`legendary launch <app>`).
#[tauri::command]
pub fn epic_launch(state: State<'_, AppState>, game_id: String) -> Result<(), String> {
    let bin = find_legendary().ok_or_else(|| LEGENDARY_MISSING.to_string())?;
    let installed = state
        .db
        .epic_states()
        .map_err(|e| e.to_string())?
        .get(&game_id)
        .map(|(_, installed)| *installed)
        .unwrap_or(false);
    if !installed {
        return Err("Gra nie jest zainstalowana.".to_string());
    }
    std::thread::spawn(move || {
        let mut cmd = cli::tool_command(&bin);
        cmd.args(["launch", game_id.as_str()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // Proces gry działa niezależnie od aplikacji — nie czekamy na jego zakończenie.
        let _ = cmd.spawn();
    });
    Ok(())
}

// ====================== TESTY ======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsuje_biblioteke_epic() {
        let db = Database::open_in_memory().unwrap();
        let raw = r#"[
            {"app_name":"Alpha","app_title":"Gra A","asset_infos":{"Windows":{}},
             "metadata":{"developer":"Studio X","keyImages":[
                {"type":"Thumbnail","url":"https://cdn.example/thumb.jpg"},
                {"type":"DieselGameBox","url":"https://cdn.example/box.jpg"}]}},
            {"app_name":"Beta","app_title":"Gra B","asset_infos":{"Windows":{}},"metadata":{}},
            {"app_name":"Dlc","app_title":"Dodatek","metadata":{"mainGameItem":{"id":"x"}}},
            {"app_name":"MacOnly","app_title":"Tylko Mac","asset_infos":{"Mac":{}},"metadata":{}},
            {"app_name":"","app_title":"Bez nazwy","metadata":{}}
        ]"#;
        let installed = r#"[
            {"app_name":"Beta","install_path":"C:/Games/Beta","version":"1.0"}
        ]"#;
        let games = parse_library(raw, installed, &db).unwrap();
        assert_eq!(games.len(), 2, "DLC, macOS-only i wpisy bez app_name odpadają");
        assert_eq!(games[0].title, "Gra A", "lista ma być posortowana po tytule");
        assert_eq!(
            games[0].cover_url.as_deref(),
            Some("https://cdn.example/box.jpg"),
            "preferujemy poziomą okładkę DieselGameBox"
        );
        assert_eq!(games[0].developer.as_deref(), Some("Studio X"));
        assert!(!games[0].installed);
        // Gra z listy zainstalowanych ma folder, ale katalog nie istnieje na dysku.
        assert!(games[1].install_dir.is_some());
        assert!(!games[1].installed);
        assert!(!games[1].dir_exists);
    }

    #[test]
    fn wyciaga_kod_logowania_epic() {
        // Pełny JSON ze strony logowania.
        assert_eq!(
            extract_epic_code(r#"{"authorizationCode":"abc123def456"}"#).as_deref(),
            Some("abc123def456")
        );
        // Sam kod.
        assert_eq!(
            extract_epic_code("  0123456789abcdef0123456789abcdef ").as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        // Adres z parametrem.
        assert_eq!(
            extract_epic_code("https://example/?authorizationCode=abc123def456&x=1").as_deref(),
            Some("abc123def456")
        );
        // Śmieci nie mogą udawać kodu.
        assert_eq!(extract_epic_code(""), None);
        assert_eq!(extract_epic_code("https://legendary.gl/epiclogin"), None);
    }
}
