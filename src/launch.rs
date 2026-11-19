//! Abrir arquivos: imagens vão para o Vislo quando ele está instalado; o resto, para o app padrão do sistema.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub fn open(path: &Path) -> Result<(), String> {
    if !path.is_dir() && vislo_core::formats::is_image(path) {
        if let Some(vislo) = vislo() {
            if spawn_vislo(vislo, path).is_ok() {
                return Ok(());
            }
        }
    }
    open::that_detached(path).map_err(|e| e.to_string())
}

/// Abre no app padrão mesmo quando é imagem (comando "Abrir com o app do sistema").
pub fn open_default(path: &Path) -> Result<(), String> {
    open::that_detached(path).map_err(|e| e.to_string())
}

/// Mostra o arquivo no gerenciador do sistema (útil para coisas que o arbor ainda não faz).
/// Ctrl+N: outra janela do arbor (outro processo), na mesma pasta ou em "Este Computador".
pub fn new_window(dir: Option<&Path>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    if let Some(d) = dir {
        cmd.arg(d);
    }
    cmd.spawn().map(drop).map_err(|e| e.to_string())
}

pub fn reveal(path: &Path) -> Result<(), String> {
    let dir = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
    open::that_detached(dir).map_err(|e| e.to_string())
}

pub fn terminal_here(dir: &Path) -> Result<(), String> {
    #[cfg(windows)]
    let result = Command::new("cmd")
        .args(["/C", "start", "", "wt", "-d"])
        .arg(dir)
        .spawn()
        .or_else(|_| Command::new("cmd").args(["/C", "start", "cmd"]).current_dir(dir).spawn());
    #[cfg(target_os = "macos")]
    let result = Command::new("open").args(["-a", "Terminal"]).arg(dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = {
        let candidates = ["x-terminal-emulator", "gnome-terminal", "konsole", "kitty", "alacritty", "wezterm", "foot", "xterm"];
        candidates
            .iter()
            .find_map(|t| Command::new(t).current_dir(dir).spawn().ok())
            .ok_or_else(|| std::io::Error::other("nenhum terminal encontrado"))
    };
    result.map(|_| ()).map_err(|e| e.to_string())
}

pub fn vislo() -> Option<&'static Path> {
    static FOUND: OnceLock<Option<PathBuf>> = OnceLock::new();
    FOUND.get_or_init(find_vislo).as_deref()
}

fn spawn_vislo(vislo: &Path, file: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    if vislo.extension().is_some_and(|e| e == "app") {
        return Command::new("open").arg("-a").arg(vislo).arg(file).spawn().map(|_| ());
    }
    Command::new(vislo).arg(file).spawn().map(|_| ())
}

fn find_vislo() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("ARBOR_VISLO").map(PathBuf::from).filter(|p| p.exists()) {
        return Some(p);
    }
    let exe = if cfg!(windows) { "vislo.exe" } else { "vislo" };
    if let Some(p) = std::env::var_os("PATH").and_then(|path| search_path(&path, exe)) {
        return Some(p);
    }
    known_locations().into_iter().find(|p| p.exists())
}

pub fn search_path(path_var: &std::ffi::OsStr, exe: &str) -> Option<PathBuf> {
    std::env::split_paths(path_var).map(|d| d.join(exe)).find(|p| p.is_file())
}

fn known_locations() -> Vec<PathBuf> {
    let mut v = Vec::new();
    #[cfg(windows)]
    {
        for var in ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(base) = std::env::var_os(var).map(PathBuf::from) {
                v.push(base.join("Vislo").join("vislo.exe"));
                v.push(base.join("Programs").join("Vislo").join("vislo.exe"));
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        v.push(PathBuf::from("/Applications/Vislo.app"));
        if let Some(home) = dirs::home_dir() {
            v.push(home.join("Applications/Vislo.app"));
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        v.push(PathBuf::from("/usr/bin/vislo"));
        v.push(PathBuf::from("/usr/local/bin/vislo"));
        if let Some(home) = dirs::home_dir() {
            v.push(home.join(".local/bin/vislo"));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acha_no_path() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("vislo"), "").unwrap();
        let path = std::env::join_paths([PathBuf::from("/nao/existe"), d.path().to_path_buf()]).unwrap();
        assert_eq!(search_path(&path, "vislo"), Some(d.path().join("vislo")));
        assert_eq!(search_path(&path, "outro"), None);
    }
}
