// goglite — wspólne wywołania zewnętrznych narzędzi
// Z tego modułu korzystają integracje ze sklepami: Amazon Games (`nile`) i Epic (`legendary`).
// Zawiera to, co jest w nich identyczne: uruchamianie procesów z limitem czasu i bez okna
// konsoli, instalację narzędzia przez winget, wyciąganie procentu/postępu z logów
// oraz wyłuskiwanie JSON-a z wyjścia narzędzia.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use tauri::{AppHandle, Emitter};

/// Ile czekamy na instalację narzędzia przez winget (pobranie + rozpakowanie).
pub const WINGET_TIMEOUT: Duration = Duration::from_secs(300);

// —————————— URUCHAMIANIE PROCESÓW ——————————

/// Buduje polecenie bez migającego okna konsoli (Windows).
pub fn tool_command(bin: &Path) -> Command {
    let mut cmd = Command::new(bin);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Wynik uruchomienia zewnętrznego procesu z limitem czasu.
pub struct RunOutcome {
    pub status: Option<ExitStatus>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl RunOutcome {
    pub fn success(&self) -> bool {
        matches!(self.status, Some(status) if status.success())
    }
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
/// Po `limit` proces jest ubijany — zawieszone nile/legendary nie blokuje aplikacji w nieskończoność.
pub fn run_with_timeout(
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

// —————————— WINGET ——————————

/// Ścieżka do `winget` — alias z `WindowsApps` bywa nieosiągalny z PATH aplikacji GUI.
pub fn winget_program() -> PathBuf {
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

/// Szuka plików `exe` w katalogach pakietów winget (instalacja „portable”).
/// Winget czasem nie zdąży utworzyć skrótu w `WinGet\Links`, więc zaglądamy też tutaj.
pub fn winget_package_exes(package_prefix: &str, exe_stem: &str) -> Vec<PathBuf> {
    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return Vec::new();
    };
    let prefix = package_prefix.to_lowercase();
    let stem = exe_stem.to_lowercase();
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
            .starts_with(&prefix)
        {
            continue;
        }
        let Ok(files) = std::fs::read_dir(dir.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().to_lowercase();
            if name.starts_with(&stem) && name.ends_with(".exe") {
                found.push(file.path());
            }
        }
    }
    found
}

/// Szuka binarki narzędzia w `GOGLITE_*` (np. `GOGLITE_NILE`), `WinGet\Links`,
/// katalogach pakietów winget i w PATH.
pub fn find_tool(env_var: &str, exe: &str, package_prefix: &str) -> Option<PathBuf> {
    if let Ok(custom) = std::env::var(env_var) {
        let path = PathBuf::from(custom);
        if path.is_file() {
            return Some(path);
        }
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(&local)
                .join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join(exe),
        );
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            candidates.push(dir.join(exe));
        }
    }
    let stem = exe.trim_end_matches(".exe");
    candidates.extend(winget_package_exes(package_prefix, stem));
    candidates.into_iter().find(|p| p.is_file())
}

/// Instaluje pakiet winget, strumieniując linie wyjścia do UI zdarzeniem `event`.
/// Bez terminala i bez uprawnień administratora.
pub fn winget_install(app: &AppHandle, event: &str, package_id: &str) -> Result<RunOutcome, String> {
    let mut cmd = tool_command(&winget_program());
    cmd.args([
        "install",
        "--exact",
        "--id",
        package_id,
        "--accept-package-agreements",
        "--accept-source-agreements",
    ]);
    run_with_timeout(cmd, WINGET_TIMEOUT, |line| {
        let line = line.trim();
        if !line.is_empty() {
            let _ = app.emit(event, serde_json::json!({ "line": line }));
        }
    })
}

// —————————— PARSOWANIE WYJŚCIA ——————————

/// Wyciąga procent z linii logu `nile` albo `legendary`.
/// nile: `INFO [PROGRESS]: = Progress: 4.62 21542473/424961033, …` (bez `%`),
/// legendary: `[DLManager] INFO: = Progress: 4.62% (1234/5678), …`,
/// obsługujemy też klasyczny format `… 42.5% …`.
pub fn parse_percent(line: &str) -> Option<f32> {
    if let Some(rest) = line.split("= Progress:").nth(1) {
        if let Some(value) = rest
            .split_whitespace()
            .next()
            .and_then(|v| v.trim_end_matches('%').parse::<f32>().ok())
        {
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

/// Zdejmuje prefiks loggera: `INFO [PROGRESS]:\t` (nile) i `[DLManager] INFO:` (legendary).
fn strip_log_prefix(line: &str) -> &str {
    const LEVELS: [&str; 6] = ["INFO", "WARNING", "ERROR", "DEBUG", "FATAL", "CRITICAL"];
    let mut rest = line.trim();
    for _ in 0..3 {
        // Poziom logu: `INFO: …` / `[Core] INFO: …`
        if let Some((head, tail)) = rest.split_once(':') {
            if LEVELS.contains(&head.trim()) {
                rest = tail.trim_start();
                continue;
            }
        }
        // Nazwa loggera: `[DLManager] …`
        if rest.starts_with('[') {
            if let Some(end) = rest.find(']') {
                rest = rest[end + 1..].trim_start();
                continue;
            }
        }
        // Znacznik zaraz po poziomie: `INFO [PROGRESS]:\t …`
        if let Some(pos) = rest.find("]:") {
            if rest[..pos]
                .chars()
                .all(|c| c.is_ascii_alphabetic() || c == '[' || c.is_whitespace())
            {
                rest = rest[pos + 2..].trim_start();
                continue;
            }
        }
        break;
    }
    rest.trim()
}

/// Zamienia linię logu na krótki tekst postępu dla UI.
/// `None` oznacza linie techniczne (prędkości zapisu, rozmiary cache), które tylko zaśmiecają listę.
pub fn progress_text(line: &str, percent: Option<f32>) -> Option<String> {
    let clean = strip_log_prefix(line);
    if clean.is_empty() || clean.starts_with('+') {
        return None;
    }
    if clean.starts_with("- Downloaded") || clean.starts_with("- Cache") {
        return None;
    }
    if percent.is_none() {
        return Some(clean.to_string());
    }
    let rest = clean.split("= Progress:").nth(1).unwrap_or("").trim();
    let pct_text = rest
        .split_whitespace()
        .next()
        .unwrap_or("0")
        .trim_end_matches('%')
        .to_string();
    // nile podaje drugi token jako `21542473/424961033`, legendary jako `(1234/5678)`.
    let done_bytes = rest
        .split_whitespace()
        .nth(1)
        .map(|size| size.trim_start_matches('('))
        .and_then(|sizes| sizes.split('/').next())
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    let eta = clean.split("ETA:").nth(1).map(str::trim).unwrap_or("");
    let mib = done_bytes / 1024.0 / 1024.0;
    let mut out = format!("{pct_text}%");
    if mib >= 0.1 {
        out.push_str(&format!(" · {mib:.1} MiB"));
    }
    if !eta.is_empty() {
        out.push_str(&format!(" · ETA {eta}"));
    }
    Some(out)
}

/// Wyłuskuje JSON z wyjścia narzędzia.
/// `nile` pisze logi na stderr (więc wystarczy `stdout`), ale `legendary` loguje na `stdout`
/// i JSON trzeba znaleźć na końcu wyjścia — także gdy jest sformatowany na wiele linii.
pub fn extract_json<T: DeserializeOwned>(raw: &str) -> Option<T> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<T>(text) {
        return Some(value);
    }
    let lines: Vec<&str> = text.lines().collect();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            let l = l.trim_start();
            l.starts_with('[') || l.starts_with('{')
        })
        .map(|(i, _)| i)
        .collect();
    for start in starts.iter().rev() {
        let candidate = lines[*start..].join("\n");
        if let Ok(value) = serde_json::from_str::<T>(&candidate) {
            return Some(value);
        }
    }
    None
}

// ====================== TESTY ======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wyciaga_procent_z_linii_logu() {
        assert_eq!(parse_percent("Downloading file 42.5% done"), Some(42.5));
        assert_eq!(parse_percent("100%"), Some(100.0));
        assert_eq!(parse_percent("bez procentow"), None);
        // Rzeczywisty format nile (bez znaku `%`).
        let nile = "INFO [PROGRESS]:\t = Progress: 4.62 21542473/424961033, Running for: 00:00:15, ETA: 00:05:12";
        assert_eq!(parse_percent(nile), Some(4.62));
        // Rzeczywisty format legendary (ze znakiem `%` i liczbami w nawiasie).
        let legendary = "[DLManager] INFO: = Progress: 12.50% (250/2000), Running for 00:02:11, ETA: 00:04:12";
        assert_eq!(parse_percent(legendary), Some(12.5));
    }

    #[test]
    fn sklada_tekst_postepu_dla_obu_narzedzi() {
        let nile = "INFO [PROGRESS]:\t = Progress: 4.62 21542473/424961033, Running for: 00:00:15, ETA: 00:05:12";
        assert_eq!(
            progress_text(nile, parse_percent(nile)).as_deref(),
            Some("4.62% · 20.5 MiB · ETA 00:05:12")
        );
        let legendary =
            "[DLManager] INFO: = Progress: 12.50% (250/2000), Running for 00:02:11, ETA: 00:04:12";
        assert_eq!(
            progress_text(legendary, parse_percent(legendary)).as_deref(),
            Some("12.50% · ETA 00:04:12")
        );
        // Prędkości zapisu/odczytu i rozmiary cache są pomijane.
        assert_eq!(
            progress_text("INFO [PROGRESS]:\t  + Disk\t- 0.75 MiB/s", None),
            None
        );
        assert_eq!(
            progress_text(
                "[DLManager] INFO:  - Downloaded: 123.45 MiB, Written: 100.00 MiB",
                None
            ),
            None
        );
        // Zwykłe komunikaty zostają, ale bez prefiksu loggera.
        assert_eq!(
            progress_text("[Core] INFO: Logging in...", None).as_deref(),
            Some("Logging in...")
        );
    }

    #[test]
    fn wyciaga_json_z_logow_na_stdout() {
        let raw = "INFO: Getting game list...\n[Core] INFO: Logged in\n[{\"app_name\":\"A\",\"app_title\":\"Gra\"}]\n";
        let parsed: Option<Vec<serde_json::Value>> = extract_json(raw);
        assert_eq!(parsed.expect("JSON na końcu wyjścia").len(), 1);
        // JSON sformatowany na wiele linii też musi się odnaleźć.
        let pretty = "INFO: start\n[\n  {\"app_name\": \"A\"}\n]\n";
        let parsed: Option<Vec<serde_json::Value>> = extract_json(pretty);
        assert_eq!(parsed.expect("wieloliniowy JSON").len(), 1);
        // Brak JSON-a nie może panikować.
        let none: Option<Vec<serde_json::Value>> = extract_json("INFO: tylko logi");
        assert!(none.is_none());
    }
}
