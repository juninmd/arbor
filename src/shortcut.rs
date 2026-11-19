//! "Criar atalho" e "Novo > Pasta compactada" do Explorer. Atalho é link simbólico no Linux/macOS e
//! `.lnk` no Windows (criar link simbólico lá exige modo de desenvolvedor ou administrador).

use std::path::{Path, PathBuf};

/// Zip vazio: só o registro de fim do diretório central, como o "Nova pasta compactada" do Windows.
pub const EMPTY_ZIP: [u8; 22] = [0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

pub fn is_zip_name(name: &str) -> bool {
    name.to_lowercase().ends_with(".zip")
}

/// "nome - Atalho" na mesma pasta do alvo (com `.lnk` no Windows), sem sobrescrever nada.
pub fn link_path(target: &Path) -> Option<PathBuf> {
    let dir = target.parent()?;
    let name = target.file_name()?.to_string_lossy();
    let ext = if cfg!(windows) { ".lnk" } else { "" };
    Some(arbor_core::ops::unique_dest(dir, &format!("{name} - Atalho{ext}")))
}

#[cfg(unix)]
pub fn create(target: &Path, link: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(target, link).map_err(|e| e.to_string())
}

/// Pelo WScript.Shell do PowerShell; os caminhos vão por variável de ambiente, nunca colados no script.
#[cfg(windows)]
pub fn create(target: &Path, link: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = "$s=(New-Object -ComObject WScript.Shell).CreateShortcut($env:ARBOR_LNK);$s.TargetPath=$env:ARBOR_TARGET;$s.Save()";
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("ARBOR_LNK", link)
        .env("ARBOR_TARGET", target)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() && link.exists() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_zip_opens_with_no_members() {
        let dir = tempfile::tempdir().unwrap();
        let z = dir.path().join("vazio.zip");
        std::fs::write(&z, EMPTY_ZIP).unwrap();
        assert!(arbor_core::archive::list(&z, Path::new("")).unwrap().is_empty());
        assert!(is_zip_name("A.ZIP") && !is_zip_name("zip.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn shortcut_points_to_the_target_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("fotos");
        std::fs::create_dir(&target).unwrap();
        let link = link_path(&target).unwrap();
        assert_eq!(link.file_name().unwrap(), "fotos - Atalho");
        create(&target, &link).unwrap();
        assert_eq!(std::fs::read_link(&link).unwrap(), target);
        assert_ne!(link_path(&target).unwrap(), link, "segundo atalho ganha outro nome");
    }
}
