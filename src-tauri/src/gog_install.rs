// goglite — instalacja i uruchamianie gier GOG z poziomu aplikacji
// Instalatory offline GOG to pakiety Inno Setup, które potrafią działać cicho:
//   /SILENT /SUPPRESSMSGBOXES /NORESTART /SP- /NOICONS /DIR="<folder>"
// Po instalacji szukamy głównego pliku gry i zapisujemy jego ścieżkę w bazie,
// dzięki czemu kolejne uruchomienie odbywa się jednym przyciskiem w aplikacji.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::State;

use crate::cli;
use crate::commands::AppState;
use crate::db::Database;

/// Instalacja dużej gry potrafi trwać bardzo długo — limit tylko po to,
/// żeby zawieszony instalator nie wisiał w nieskończoność.
const INSTALL_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// Czy nazwa pliku to główny instalator (a nie część `.bin`).
fn is_setup_file(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".exe")
}

/// Czyszczenie tytułu do nazwy folderu (bez znaków niedozwolonych w Windows).
fn sanitize_dir_name(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| if matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.').to_string();
    if cleaned.is_empty() { "GOG Game".to_string() } else { cleaned }
}

/// Pliki pomocnicze (redistributable, odinstalowanie) — nie są grą.
fn is_side_file(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["unins", "setup", "vcredist", "dxsetup", "dxwebsetup", "dotnet", "redist", "galaxy"]
        .iter()
        .any(|bad| name.contains(bad))
}

/// Szuka głównego pliku exe zainstalowanej gry (największy, pomijając pomocnicze).
fn find_game_exe(root: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > 3 {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                // Katalogi z dodatkami (DirectX, VC++) tylko zaśmiecają wynik.
                if name.contains("redist") || name.contains("directx") || name.contains("__installer") {
                    continue;
                }
                stack.push((path, depth + 1));
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.to_ascii_lowercase().ends_with(".exe") || is_side_file(&name) {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if best.as_ref().map(|(b, _)| size > *b).unwrap_or(true) {
                best = Some((size, path));
            }
        }
    }
    best.map(|(_, path)| path)
}

/// Tytuł gry z bazy (do nazwy folderu instalacji).
fn game_title(db: &Database, game_id: i64) -> String {
    db.list_games()
        .unwrap_or_default()
        .into_iter()
        .find(|g| g.id == game_id)
        .map(|g| g.title)
        .unwrap_or_else(|| format!("Gra {game_id}"))
}

/// Cicha instalacja gry z pobranego instalatora offline.
/// Zwraca ścieżkę pliku exe zapisanego dla gry.
fn install_game(db: &Database, default_dir: &Path, game_id: i64) -> Result<String, String> {
    let rows: Vec<_> = db
        .list_downloads()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|r| r.game_id == game_id)
        .collect();
    // Instalujemy z instalatora, którego wszystkie pliki (także części `.bin`) są pobrane.
    let setup = rows
        .iter()
        .find(|r| {
            r.status == "completed"
                && r.file_exists
                && is_setup_file(&r.filename)
                && rows
                    .iter()
                    .filter(|x| x.installer_id == r.installer_id)
                    .all(|x| x.status == "completed" && x.file_exists)
        })
        .ok_or_else(|| "Najpierw pobierz wszystkie pliki instalatora.".to_string())?;

    let setup_path = PathBuf::from(&setup.target_path);
    let setup_dir = setup_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Nieprawidłowa ścieżka instalatora.".to_string())?;

    // Folder wybrany dla gry ma pierwszeństwo; instalujemy w podfolderze z tytułem,
    // żeby pliki gry nie mieszały się z instalatorami.
    let base = db
        .get_game_install_dir(game_id)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_dir.to_path_buf());
    let target = base.join(sanitize_dir_name(&game_title(db, game_id)));
    std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;

    let mut cmd = cli::tool_command(&setup_path);
    cmd.current_dir(&setup_dir)
        .arg("/SILENT")
        .arg("/SUPPRESSMSGBOXES")
        .arg("/NORESTART")
        .arg("/SP-")
        .arg("/NOICONS")
        // Bez cudzysłowów w argumencie: Rust sam opakuje go w cudzysłowy,
        // a Inno Setup nie rozumie escapowanych `\"` i dostawał ścieżkę z backslashami.
        .arg(format!("/DIR={}", target.display()));
    let out = cli::run_with_timeout(cmd, INSTALL_TIMEOUT, |_| {})?;
    if out.timed_out {
        return Err("Instalator nie zakończył pracy w wyznaczonym czasie.".to_string());
    }
    // Kod wyjścia instalatorów GOG bywa mylący: potrafią zainstalować grę w całości
    // i zakończyć proces kodem błędu (np. 0xC000041D przy uruchamianiu `__redist`).
    // Dlatego rozstrzyga stan plików — brak gry to dopiero prawdziwy błąd.
    let Some(exe) = find_game_exe(&target) else {
        let code = out
            .status
            .map(|s| s.code().map(|c| c.to_string()).unwrap_or_else(|| "?".into()))
            .unwrap_or_else(|| "?".into());
        let output = format!("{} {}", out.stdout, out.stderr);
        let last = output.trim().lines().last().unwrap_or("").trim();
        return Err(if last.is_empty() {
            format!("Instalator zakończył się błędem (kod {code}) i nie zainstalował gry.")
        } else {
            format!("Instalator zakończył się błędem (kod {code}): {last}")
        });
    };
    if !out.success() {
        log::warn!(
            "Instalator GOG zwrócił błąd, ale gra jest na dysku: {}",
            exe.display()
        );
    }
    let exe_str = exe.to_string_lossy().to_string();
    db.set_game_launch_exe(game_id, Some(&exe_str))
        .map_err(|e| e.to_string())?;
    // Waga instalacji = realne zajęte miejsce po instalacji.
    let _ = db.set_game_size_bytes(game_id, cli::dir_size(&target));
    Ok(exe_str)
}

/// Instaluje grę GOG z pobranego instalatora (cicho) i zwraca ścieżkę exe.
#[tauri::command]
pub async fn gog_install(state: State<'_, AppState>, game_id: i64) -> Result<String, String> {
    let db = state.db.clone();
    let default_dir = state.download_dir.lock().unwrap().clone();
    tauri::async_runtime::spawn_blocking(move || install_game(&db, &default_dir, game_id))
        .await
        .map_err(|e| e.to_string())?
}

/// Uruchamia zainstalowaną grę GOG (exe zapisany przy instalacji).
#[tauri::command]
pub fn gog_launch(state: State<'_, AppState>, game_id: i64) -> Result<(), String> {
    let exe = state
        .db
        .get_game_launch_exe(game_id)
        .ok_or_else(|| "Gra nie jest zainstalowana z poziomu goglite.".to_string())?;
    let path = PathBuf::from(&exe);
    if !path.is_file() {
        return Err("Plik gry nie istnieje — zainstaluj ją ponownie.".to_string());
    }
    let mut cmd = cli::tool_command(&path);
    if let Some(dir) = path.parent() {
        cmd.current_dir(dir);
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // Gra działa niezależnie od aplikacji — nie czekamy na jej zakończenie.
    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}

// ====================== TESTY ======================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rozpoznaje_glowny_instalator() {
        assert!(is_setup_file("setup_gra_1.0_(12345).exe"));
        assert!(!is_setup_file("setup_gra_1.0_(12345)-1.bin"));
        assert!(is_setup_file("SETUP.EXE"));
    }

    #[test]
    fn pomija_pliki_pomocnicze() {
        assert!(is_side_file("unins000.exe"));
        assert!(is_side_file("vcredist_x64.exe"));
        assert!(is_side_file("DXSETUP.exe"));
        assert!(!is_side_file("Gra.exe"));
    }

    #[test]
    fn czysci_nazwe_folderu() {
        assert_eq!(sanitize_dir_name("Diablo II: Resurrected"), "Diablo II_ Resurrected");
        assert_eq!(sanitize_dir_name("  "), "GOG Game");
        assert_eq!(sanitize_dir_name("Wiedźmin 3."), "Wiedźmin 3");
    }
}
