// goglite — integracja z Amazon Games (Prime Gaming)
// Amazon nie udostępnia publicznego API dla klienta gier, więc korzystamy z `nile`
// (otwarty klient Amazon Games, ten sam, którego używa Heroic i Lutris):
//   nile auth --login --non-interactive / --status / --logout
//   nile register --code <kod> --client-id <id> --code-verifier <verifier> --serial <serial>
//   nile library list --json
//   nile install <id> --path <folder>
//   nile launch <id>
// Dzięki temu logowanie, pobieranie i uruchamianie obsługuje dojrzałe narzędzie,
// a goglite dokłada UI, foldery per gra i stan instalacji.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::commands::AppState;
use crate::db::Database;

/// Komunikat zwracany, gdy `nile` nie jest zainstalowany.
const NILE_MISSING: &str =
    "Nie znaleziono narzędzia nile. Zainstaluj je komendą: winget install --id imLinguin.nile";

/// Ile czekamy na odpowiedź `nile` (status, logowanie, wypisanie biblioteki).
const NILE_READ_TIMEOUT: Duration = Duration::from_secs(120);
/// Synchronizacja biblioteki w sieci bywa dłuższa — osobny limit.
const NILE_SYNC_TIMEOUT: Duration = Duration::from_secs(300);
/// Instalacja przez winget (pobranie pakietu i rozpakowanie).
const WINGET_TIMEOUT: Duration = Duration::from_secs(300);

/// Stan integracji widoczny w UI.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonStatus {
    /// Czy `nile` jest dostępny na tym komputerze.
    pub available: bool,
    /// Ścieżka do znalezionego pliku `nile`.
    pub nile_path: Option<String>,
    pub logged_in: bool,
    pub username: Option<String>,
}

/// Gra z biblioteki Amazon Games (połączona z lokalnym stanem instalacji).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmazonGame {
    pub id: String,
    pub title: String,
    pub genre: Option<String>,
    /// Adres okładki (Amazon podaje panoramiczne 16:9 w `backgroundUrl1`).
    pub cover_url: Option<String>,
    pub install_dir: Option<String>,
    pub installed: bool,
    /// Czy wybrany folder nadal istnieje (gra mogła zostać usunięta z dysku).
    pub dir_exists: bool,
}

/// Dane logowania wypisane przez `nile auth --login --non-interactive`.
/// Używamy ich później w `nile register`, po wklejeniu adresu przekierowania.
#[derive(Debug, Clone, Deserialize)]
struct NileLoginData {
    client_id: String,
    code_verifier: String,
    serial: String,
    url: String,
}

static PENDING_LOGIN: OnceLock<Mutex<Option<NileLoginData>>> = OnceLock::new();

fn pending_login() -> &'static Mutex<Option<NileLoginData>> {
    PENDING_LOGIN.get_or_init(|| Mutex::new(None))
}

// —————————— URUCHAMIANIE NILE ——————————

/// Buduje polecenie `nile` bez migającego okna konsoli (Windows).
fn nile_command(bin: &Path) -> Command {
    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Szuka `nile` w zmiennej `GOGLITE_NILE`, typowych miejscach instalacji i w PATH.
fn find_nile() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("GOGLITE_NILE") {
        let path = PathBuf::from(custom);
        if path.is_file() {
            return Some(path);
        }
    }

    let exe = if cfg!(windows) { "nile.exe" } else { "nile" };
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(PathBuf::from(&local).join("Microsoft").join("WinGet").join("Links").join(exe));
    }
    if let Ok(roaming) = std::env::var("APPDATA") {
        candidates.push(PathBuf::from(&roaming).join("Python").join("Scripts").join(exe));
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        candidates.push(
            PathBuf::from(&home).join("AppData").join("Local").join("Programs").join("Python").join("Python312").join("Scripts").join(exe),
        );
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            candidates.push(dir.join(exe));
        }
    }
    candidates.extend(winget_package_exes());
    candidates.into_iter().find(|p| p.is_file())
}

/// Szuka `nile.exe` w katalogach pakietów winget (instalacja „portable”).
/// Winget czasem nie zdąży utworzyć skrótu w `WinGet\Links`, więc zaglądamy też tutaj.
fn winget_package_exes() -> Vec<PathBuf> {
    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return Vec::new();
    };
    let root = PathBuf::from(local)
        .join("Microsoft")
        .join("WinGet")
        .join("Packages");
    let Ok(dirs) = std::fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for dir in dirs.flatten() {
        if !dir
            .file_name()
            .to_string_lossy()
            .to_lowercase()
            .starts_with("imlinguin.nile")
        {
            continue;
        }
        let Ok(files) = std::fs::read_dir(dir.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("nile") && name.ends_with(".exe") {
                found.push(file.path());
            }
        }
    }
    found
}

// —————————— WYWOŁYWANIE ZEWNĘTRZNYCH NARZĘDZI ——————————

/// Wynik uruchomienia zewnętrznego procesu z limitem czasu.
struct RunOutcome {
    status: Option<std::process::ExitStatus>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

fn push_line(
    is_err: bool,
    line: String,
    stdout: &mut String,
    stderr: &mut String,
    on_line: &mut dyn FnMut(&str),
) {
    on_line(&line);
    let target = if is_err { stderr } else { stdout };
    target.push_str(&line);
    target.push('\n');
}

/// Uruchamia proces, zbierając `stdout`/`stderr`; każda linia trafia też do `on_line`.
/// Po `limit` proces jest ubijany — zawieszone winget/nile nie blokuje aplikacji w nieskończoność.
fn run_with_timeout(
    mut cmd: Command,
    limit: Duration,
    mut on_line: impl FnMut(&str),
) -> Result<RunOutcome, String> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Nie udało się uruchomić narzędzia: {e}"))?;

    // Osobne wątki czytające — inaczej pełny bufor rury zablokowałby proces.
    let (tx, rx) = mpsc::channel::<(bool, String)>();
    let mut pipes: Vec<(bool, Box<dyn std::io::Read + Send>)> = Vec::new();
    if let Some(pipe) = child.stdout.take() {
        pipes.push((false, Box::new(pipe)));
    }
    if let Some(pipe) = child.stderr.take() {
        pipes.push((true, Box::new(pipe)));
    }
    for (is_err, pipe) in pipes {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(pipe).lines() {
                let Ok(line) = line else { break };
                if tx.send((is_err, line)).is_err() {
                    break;
                }
            }
        });
    }
    drop(tx);

    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut status = None;
    let mut timed_out = false;
    let deadline = Instant::now() + limit;
    loop {
        while let Ok((is_err, line)) = rx.recv_timeout(Duration::from_millis(40)) {
            push_line(is_err, line, &mut stdout, &mut stderr, &mut on_line);
        }
        match child.try_wait() {
            Ok(Some(st)) => {
                status = Some(st);
                break;
            }
            Ok(None) => {}
            Err(e) => return Err(format!("Błąd procesu: {e}")),
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
    }

    // Wyjście, które doszło już po zakończeniu procesu.
    let flush_until = Instant::now() + Duration::from_millis(500);
    while Instant::now() < flush_until {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok((is_err, line)) => push_line(is_err, line, &mut stdout, &mut stderr, &mut on_line),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }

    Ok(RunOutcome {
        status,
        stdout,
        stderr,
        timed_out,
    })
}

/// Ścieżka do `winget` — alias z `WindowsApps` bywa nieosiągalny z PATH aplikacji GUI.
fn winget_program() -> PathBuf {
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let alias = PathBuf::from(local)
            .join("Microsoft")
            .join("WindowsApps")
            .join("winget.exe");
        if alias.is_file() {
            return alias;
        }
    }
    PathBuf::from("winget")
}

// —————————— INSTALACJA NILE (winget) ——————————

/// Instaluje `nile` przez winget — bez terminala i bez uprawnień administratora.
/// Postęp (linie winget) leci zdarzeniem `amazon:nile-progress`, żeby okno nie wyglądało na zawieszone.
fn install_nile_via_winget(app: &AppHandle) -> Result<(), String> {
    if find_nile().is_some() {
        return Ok(());
    }
    let mut cmd = Command::new(winget_program());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.args([
        "install",
        "--exact",
        "--id",
        "imLinguin.nile",
        "--accept-package-agreements",
        "--accept-source-agreements",
    ]);
    let out = run_with_timeout(cmd, WINGET_TIMEOUT, |line| {
        let line = line.trim();
        if !line.is_empty() {
            let _ = app.emit("amazon:nile-progress", serde_json::json!({ "line": line }));
        }
    })?;

    // Winget potrafi zwrócić błąd „już zainstalowano” — rozstrzyga obecność pliku.
    if find_nile().is_some() {
        return Ok(());
    }
    if out.timed_out {
        return Err(
            "Instalacja nile przez winget trwała zbyt długo i została przerwana. Spróbuj ponownie albo zainstaluj ręcznie: winget install --id imLinguin.nile"
                .to_string(),
        );
    }
    if matches!(out.status, Some(status) if status.success()) {
        return Err(
            "nile został zainstalowany, ale nie znaleziono pliku nile.exe — uruchom goglite ponownie."
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
        "winget nie zainstalował nile.".to_string()
    } else {
        msg
    })
}

/// Instaluje `nile` przyciskiem w aplikacji i zwraca nowy stan integracji.
#[tauri::command]
pub async fn amazon_install_nile(app: AppHandle) -> Result<AmazonStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_nile_via_winget(&app)?;
        let status = status_snapshot();
        if !status.available {
            return Err("Nie znaleziono nile po instalacji.".to_string());
        }
        Ok(status)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Uruchamia `nile` i zwraca `stdout` (błąd procesu → komunikat z `stderr`).
fn run_nile(args: &[&str], limit: Duration) -> Result<String, String> {
    let bin = find_nile().ok_or_else(|| NILE_MISSING.to_string())?;
    let mut cmd = nile_command(&bin);
    cmd.args(args);
    let out = run_with_timeout(cmd, limit, |_| {})?;
    if out.timed_out {
        return Err(format!(
            "nile {} nie odpowiedział w wyznaczonym czasie",
            args.join(" ")
        ));
    }
    if !matches!(out.status, Some(status) if status.success()) {
        let err = out.stderr.trim().to_string();
        return Err(if err.is_empty() {
            format!("nile {} zakończył się błędem", args.join(" "))
        } else {
            err
        });
    }
    Ok(out.stdout)
}

// —————————— STATUS / LOGOWANIE ——————————

#[derive(Deserialize)]
struct NileAuthStatus {
    #[serde(rename = "LoggedIn")]
    logged_in: bool,
    #[serde(rename = "Username")]
    username: String,
}

/// Odczytuje stan logowania z `nile auth --status` (drukuje jedną linię JSON).
fn auth_status() -> (bool, Option<String>) {
    let Ok(raw) = run_nile(&["auth", "--status"], NILE_READ_TIMEOUT) else {
        return (false, None);
    };
    let line = raw.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
    match serde_json::from_str::<NileAuthStatus>(line.trim()) {
        Ok(st) if st.logged_in => (true, Some(st.username)),
        _ => (false, None),
    }
}

/// Odczytuje stan integracji (uruchamia `nile` — tylko poza wątkiem UI).
fn status_snapshot() -> AmazonStatus {
    let Some(bin) = find_nile() else {
        return AmazonStatus {
            available: false,
            nile_path: None,
            logged_in: false,
            username: None,
        };
    };
    let (logged_in, username) = auth_status();
    AmazonStatus {
        available: true,
        nile_path: Some(bin.to_string_lossy().to_string()),
        logged_in,
        username,
    }
}

/// Zwraca informacje o dostępności `nile` i zalogowanym koncie Amazon.
/// Komenda jest `async`, bo start Pythona (nile) blokowałby wątek UI i zawieszał okno.
#[tauri::command]
pub async fn amazon_status() -> AmazonStatus {
    tauri::async_runtime::spawn_blocking(status_snapshot)
        .await
        .unwrap_or_else(|_| AmazonStatus {
            available: false,
            nile_path: None,
            logged_in: false,
            username: None,
        })
}

/// Start logowania konta Amazon.
///
/// `nile auth --login` czyta dane z terminala (`input()`), więc uruchomione bez konsoli
/// wywala się błędem `EOFError`. Dlatego wołamy je z `--non-interactive`: nile wypisuje
/// wtedy JSON z adresem logowania i danymi potrzebnymi do `nile register`.
/// Adres otwieramy w przeglądarce, a użytkownik wkleja adres przekierowania w UI
/// (`amazon_login_complete`) — bez terminala i bez blokowania wątku UI.
#[tauri::command]
pub async fn amazon_login(app: AppHandle) -> Result<(), String> {
    let bin = find_nile().ok_or_else(|| NILE_MISSING.to_string())?;
    let data = tauri::async_runtime::spawn_blocking(move || -> Result<Option<NileLoginData>, String> {
        let mut cmd = nile_command(&bin);
        cmd.args(["auth", "--login", "--non-interactive"])
            .stdin(Stdio::null());
        let out = run_with_timeout(cmd, NILE_READ_TIMEOUT, |_| {})?;
        if out.timed_out {
            return Err("nile nie odpowiedział w wyznaczonym czasie".to_string());
        }
        let line = out
            .stdout
            .lines()
            .rev()
            .map(str::trim)
            .find(|l| l.starts_with('{'));
        if let Some(line) = line {
            let data = serde_json::from_str::<NileLoginData>(line)
                .map_err(|e| format!("Nie udało się odczytać danych logowania nile: {e}"))?;
            return Ok(Some(data));
        }
        // Starsze wersje nile nie znają `--non-interactive` — podpowiadamy aktualizację.
        if out.stderr.contains("unrecognized arguments") {
            return Err(
                "Twoja wersja nile nie obsługuje logowania bez terminala. Zaktualizuj nile: winget upgrade --id imLinguin.nile"
                    .to_string(),
            );
        }
        // Brak JSON-a najczęściej oznacza, że konto jest już zalogowane.
        if auth_status().0 {
            return Ok(None);
        }
        let msg = out.stderr.trim();
        Err(if msg.is_empty() {
            "Logowanie do Amazon nie powiodło się.".to_string()
        } else {
            msg.to_string()
        })
    })
    .await
    .map_err(|e| e.to_string())??;

    let Some(data) = data else {
        return Ok(());
    };
    let url = data.url.clone();
    *pending_login().lock().unwrap() = Some(data);
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_url(url, None::<&str>)
        .map_err(|e| format!("Nie udało się otworzyć przeglądarki: {e}"))?;
    Ok(())
}

/// Wyciąga `openid.oa2.authorization_code` z adresu przekierowania Amazon.
fn authorization_code(redirect: &str) -> Option<String> {
    const KEY: &str = "openid.oa2.authorization_code=";
    let text = redirect.trim();
    if text.is_empty() {
        return None;
    }
    let raw = if let Some(pos) = text.find(KEY) {
        text[pos + KEY.len()..]
            .split(['&', '#', ' ', '\n', '\r', '"', '<', '>'])
            .next()
            .unwrap_or("")
    } else if !text.contains("://") && !text.contains('=') && !text.contains(char::is_whitespace) {
        // Użytkownik wkleił sam kod zamiast całego adresu.
        text
    } else {
        return None;
    };
    let code = percent_decode(raw);
    let code = code.trim().to_string();
    if code.is_empty() {
        None
    } else {
        Some(code)
    }
}

/// Dekoduje sekwencje `%XX` (Amazon koduje część znaków kodu autoryzacji).
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Dokańcza logowanie: rejestruje urządzenie (`nile register`) na podstawie adresu,
/// na który przekierowała przeglądarka po zalogowaniu.
#[tauri::command]
pub async fn amazon_login_complete(redirect: String) -> Result<AmazonStatus, String> {
    let data = pending_login().lock().unwrap().take().ok_or_else(|| {
        "Najpierw kliknij „Zaloguj do Amazon”, aby otworzyć stronę logowania.".to_string()
    })?;
    let code = authorization_code(&redirect).ok_or_else(|| {
        *pending_login().lock().unwrap() = Some(data.clone());
        "Nie znaleziono kodu w adresie. Skopiuj pełny adres strony amazon.com z paska przeglądarki — zawiera on parametr openid.oa2.authorization_code."
            .to_string()
    })?;
    tauri::async_runtime::spawn_blocking(move || -> Result<AmazonStatus, String> {
        let result = run_nile(
            &[
                "register",
                "--code",
                &code,
                "--client-id",
                &data.client_id,
                "--code-verifier",
                &data.code_verifier,
                "--serial",
                &data.serial,
            ],
            NILE_SYNC_TIMEOUT,
        );
        // Rejestracja mogła się udać, a błąd dotyczyć dopiero synchronizacji biblioteki.
        let status = status_snapshot();
        if status.logged_in {
            return Ok(status);
        }
        result?;
        Err("Amazon nie potwierdził logowania. Spróbuj ponownie — wklej adres, na który przekierowała przeglądarka po zalogowaniu."
            .to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Wylogowanie konta Amazon (`nile auth --logout`).
#[tauri::command]
pub async fn amazon_logout() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        run_nile(&["auth", "--logout"], NILE_READ_TIMEOUT).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

// —————————— BIBLIOTEKA ——————————

/// Parsuje `nile library list --json` i dokłada lokalny folder/stan instalacji.
fn parse_library(raw: &str, db: &Database) -> Result<Vec<AmazonGame>, String> {
    let value: serde_json::Value = serde_json::from_str(raw.trim())
        .map_err(|e| format!("Nie udało się odczytać biblioteki nile: {e}"))?;
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    let states = db.amazon_states().map_err(|e| e.to_string())?;
    let mut games = Vec::new();
    for item in items {
        let product = &item["product"];
        let Some(id) = product["id"].as_str().filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        let Some(title) = product["title"].as_str().filter(|s| !s.trim().is_empty()) else {
            continue;
        };
        let genre = product["productDetail"]["details"]["genres"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let detail = &product["productDetail"];
        let cover_url = detail["details"]["backgroundUrl1"]
            .as_str()
            .or_else(|| detail["iconUrl"].as_str())
            .filter(|s| s.starts_with("http"))
            .map(|s| s.to_string());
        let (install_dir, installed) = states.get(id).cloned().unwrap_or((None, false));
        let dir_exists = install_dir
            .as_deref()
            .map(|d| Path::new(d).is_dir())
            .unwrap_or(false);
        games.push(AmazonGame {
            id: id.to_string(),
            title: title.to_string(),
            genre,
            cover_url,
            install_dir,
            installed: installed && dir_exists,
            dir_exists,
        });
    }
    games.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(games)
}

/// Biblioteka Amazon Games z lokalnego cache `nile` (bez pytania sieci).
#[tauri::command]
pub async fn amazon_library(state: State<'_, AppState>) -> Result<Vec<AmazonGame>, String> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let raw = run_nile(&["library", "list", "--json"], NILE_READ_TIMEOUT)?;
        parse_library(&raw, &db)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Wymusza ponowną synchronizację biblioteki Amazon (`nile library sync --force`).
#[tauri::command]
pub async fn amazon_sync() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        run_nile(&["library", "sync", "--force"], NILE_SYNC_TIMEOUT).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

// —————————— FOLDER GRY ——————————

/// Wybór folderu instalacji dla konkretnej gry Amazon; zapisuje go w bazie.
#[tauri::command]
pub async fn pick_amazon_dir(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let start = state
        .db
        .get_amazon_install_dir(&game_id)
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
        .set_amazon_install_dir(&game_id, Some(&dir))
        .map_err(|e| e.to_string())?;
    Ok(Some(dir))
}

// —————————— POBIERANIE / URUCHAMIANIE ——————————

/// Wyciąga procent z linii logu `nile`.
/// nile wypisuje postęp jako `INFO [PROGRESS]: = Progress: 4.62 21542473/424961033, …`
/// (bez znaku `%`), więc obsługujemy też klasyczny format `… 42.5% …`.
fn parse_percent(line: &str) -> Option<f32> {
    if let Some(rest) = line.split("= Progress:").nth(1) {
        if let Some(value) = rest.split_whitespace().next().and_then(|v| v.parse::<f32>().ok()) {
            return Some(value);
        }
    }
    let idx = line.find('%')?;
    let head = &line[..idx];
    let number: String = head
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    number.parse::<f32>().ok()
}

/// Zamienia linię logu `nile` na krótki tekst dla UI.
/// `None` oznacza linie techniczne (prędkości odczytu/zapisu), które tylko zaśmiecają listę.
fn progress_text(line: &str, percent: Option<f32>) -> Option<String> {
    let clean = line
        .split_once("]:\t")
        .map(|(_, rest)| rest)
        .unwrap_or(line)
        .trim();
    if clean.is_empty() || clean.starts_with('+') {
        return None;
    }
    if percent.is_some() {
        let rest = clean.split("= Progress:").nth(1).unwrap_or("").trim();
        let mut parts = rest.split_whitespace();
        let pct = parts.next().unwrap_or("0").to_string();
        let done_bytes = parts
            .next()
            .and_then(|sizes| sizes.split('/').next())
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let eta = clean.split("ETA:").nth(1).map(str::trim).unwrap_or("");
        let mib = done_bytes / 1024.0 / 1024.0;
        return Some(if eta.is_empty() {
            format!("{pct}% · {mib:.1} MiB")
        } else {
            format!("{pct}% · {mib:.1} MiB · ETA {eta}")
        });
    }
    Some(clean.to_string())
}

fn emit_done(app: &AppHandle, game_id: &str, ok: bool, error: &str) {
    let _ = app.emit(
        "amazon:done",
        serde_json::json!({ "gameId": game_id, "ok": ok, "error": error }),
    );
}

/// Pobiera i instaluje grę przez `nile install <id> --path <folder>`.
/// Postęp leci zdarzeniami `amazon:progress`, zakończenie — `amazon:done`.
#[tauri::command]
pub fn amazon_install(
    app: AppHandle,
    state: State<'_, AppState>,
    game_id: String,
) -> Result<(), String> {
    let bin = find_nile().ok_or_else(|| NILE_MISSING.to_string())?;
    let dir = state
        .db
        .get_amazon_install_dir(&game_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Najpierw wybierz folder instalacji dla tej gry.".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let db = state.db.clone();
    let _ = db.set_amazon_installed(&game_id, false);

    std::thread::spawn(move || {
        let mut cmd = nile_command(&bin);
        let mut child = match cmd
            .args(["install", game_id.as_str(), "--path", dir.as_str()])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(e) => {
                emit_done(&app, &game_id, false, &format!("Nie udało się uruchomić nile: {e}"));
                return;
            }
        };

        let mut last_line = String::new();
        if let Some(stderr) = child.stderr.take() {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                let percent = parse_percent(&line);
                let Some(text) = progress_text(&line, percent) else { continue };
                last_line = text.clone();
                let _ = app.emit(
                    "amazon:progress",
                    serde_json::json!({ "gameId": game_id, "line": text, "percent": percent }),
                );
            }
        }

        let ok = matches!(child.wait(), Ok(status) if status.success());
        if ok {
            let _ = db.set_amazon_installed(&game_id, true);
        }
        let error = if ok { String::new() } else { last_line };
        emit_done(&app, &game_id, ok, &error);
    });
    Ok(())
}

/// Uruchamia zainstalowaną grę (`nile launch <id>`).
#[tauri::command]
pub fn amazon_launch(state: State<'_, AppState>, game_id: String) -> Result<(), String> {
    let bin = find_nile().ok_or_else(|| NILE_MISSING.to_string())?;
    let installed = state
        .db
        .amazon_states()
        .map_err(|e| e.to_string())?
        .get(&game_id)
        .map(|(_, installed)| *installed)
        .unwrap_or(false);
    if !installed {
        return Err("Gra nie jest zainstalowana.".to_string());
    }
    std::thread::spawn(move || {
        let mut cmd = nile_command(&bin);
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
    fn parsuje_biblioteke_nile() {
        let db = Database::open_in_memory().unwrap();
        let raw = r#"[
            {"product":{"id":"abc","title":"Gra B","productDetail":{"details":{"genres":["Action"]}}}},
            {"product":{"id":"xyz","title":"Gra A","productDetail":{"details":{"backgroundUrl1":"https://m.media-amazon.com/images/I/test.jpg"}}}},
            {"product":{"id":"","title":"Bez id"}}
        ]"#;
        let games = parse_library(raw, &db).unwrap();
        assert_eq!(games.len(), 2, "wpisy bez id muszą być pomijane");
        assert_eq!(games[0].title, "Gra A", "lista ma być posortowana po tytule");
        assert_eq!(games[1].genre.as_deref(), Some("Action"));
        assert!(!games[0].installed);
        assert_eq!(
            games[0].cover_url.as_deref(),
            Some("https://m.media-amazon.com/images/I/test.jpg"),
            "okładka ma pochodzić z backgroundUrl1"
        );
    }

    #[test]
    fn wyciaga_procent_z_linii_logu() {
        assert_eq!(parse_percent("Downloading file 42.5% done"), Some(42.5));
        assert_eq!(parse_percent("100%"), Some(100.0));
        assert_eq!(parse_percent("bez procentow"), None);
        // Rzeczywisty format nile (bez znaku `%`).
        let line = "INFO [PROGRESS]:\t = Progress: 4.62 21542473/424961033, Running for: 00:00:15, ETA: 00:05:12";
        assert_eq!(parse_percent(line), Some(4.62));
        assert_eq!(
            progress_text(line, parse_percent(line)).as_deref(),
            Some("4.62% · 20.5 MiB · ETA 00:05:12")
        );
        // Prędkości zapisu/odczytu są pomijane.
        assert_eq!(
            progress_text("INFO [PROGRESS]:\t  + Disk\t- 0.75 MiB/s", None),
            None
        );
    }

    #[test]
    fn wyciaga_kod_z_adresu_przekierowania() {
        let url = "https://www.amazon.com/?openid.oa2.authorization_code=ANT3AbC%2BdEf&openid.assoc_handle=amzn";
        assert_eq!(authorization_code(url).as_deref(), Some("ANT3AbC+dEf"));
        // Sam kod też jest akceptowany.
        assert_eq!(authorization_code(" ANT3AbCdEf ").as_deref(), Some("ANT3AbCdEf"));
        // Adres bez kodu jest odrzucany.
        assert_eq!(authorization_code("https://www.amazon.com/"), None);
        assert_eq!(authorization_code(""), None);
    }
}
