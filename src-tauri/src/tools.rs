// goglite — wymagane narzędzia zewnętrzne
// Aplikacja przy starcie sprawdza, czy `nile` (Amazon Games) i `legendary` (Epic)
// są dostępne, i doinstalowuje brakujące przez winget. Dzięki temu użytkownik nie
// musi robić tego ręcznie po każdej instalacji czy aktualizacji aplikacji.
// Instalacja leci w tle i nie blokuje uruchomienia okna.

use serde::Serialize;
use tauri::AppHandle;

use crate::{amazon, epic};

/// Wynik sprawdzenia narzędzi — trafia do UI, które odświeża widoki sklepów.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreTools {
    pub amazon_ready: bool,
    pub epic_ready: bool,
    /// Narzędzia doinstalowane w tej sesji (`nile`, `legendary`).
    pub installed: Vec<String>,
}

/// Sprawdza oba narzędzia i instaluje brakujące.
/// Błędy instalacji nie przerywają startu — widok sklepu pokaże wtedy panel instalacji.
#[tauri::command]
pub async fn ensure_store_tools(app: AppHandle) -> StoreTools {
    tauri::async_runtime::spawn_blocking(move || {
        let mut installed = Vec::new();
        if amazon::find_nile().is_none() && amazon::install_nile_via_winget(&app).is_ok() {
            installed.push("nile".to_string());
        }
        if epic::find_legendary().is_none() && epic::install_legendary_via_winget(&app).is_ok() {
            installed.push("legendary".to_string());
        }
        StoreTools {
            amazon_ready: amazon::find_nile().is_some(),
            epic_ready: epic::find_legendary().is_some(),
            installed,
        }
    })
    .await
    .unwrap_or_else(|_| StoreTools {
        amazon_ready: false,
        epic_ready: false,
        installed: Vec::new(),
    })
}
