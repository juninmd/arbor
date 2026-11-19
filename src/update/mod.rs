//! Atualização automática a partir das releases deste repositório (a mesma abordagem do Vislo).
//!
//! ```text
//! início + 4 s / a cada 6 h ─► GET releases/latest ─► tag mais nova? ─► aviso "arbor X disponível"
//!                                                                         │ Atualizar e reiniciar
//!   arbor novo abre ◄── troca o executável ◄── SHA-256 confere? ◄── baixa arbor-X-<so>-<arq>
//! ```
//!
//! Só HTTPS, só links de `github.com/juninmd/arbor/releases/download/`, e o binário é descartado se não bater
//! com o `SHA256SUMS.txt` da release. A troca usa `self-replace`, que funciona com o executável aberto também
//! no Windows. Cópias em pastas sem permissão de escrita (pacotes do sistema) só recebem o link da release.

mod release;

pub use release::Update;

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

const API: &str = "https://api.github.com/repos/juninmd/arbor/releases/latest";
const DOWNLOAD_PREFIX: &str = "https://github.com/juninmd/arbor/releases/download/";
const MAX_BYTES: u64 = 200 * 1024 * 1024;
const USER_AGENT: &str = concat!("arbor/", env!("CARGO_PKG_VERSION"));

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Primeira verificação depois que a janela já abriu e a pasta foi desenhada: nunca atrasa a abertura.
pub const FIRST_CHECK: Duration = Duration::from_secs(4);
pub const EVERY: Duration = Duration::from_secs(6 * 60 * 60);

static CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone)]
pub enum Progress {
    Downloading {
        received: u64,
        total: Option<u64>,
    },
    Verifying,
    /// O executável novo já está no lugar; falta só reiniciar.
    Ready,
    Failed(String),
}

fn os() -> &'static str {
    std::env::consts::OS
}

fn arch() -> &'static str {
    std::env::consts::ARCH
}

fn agent(timeout: Option<Duration>) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(true) // vale para cada redirecionamento também
        .max_redirects(5)
        .timeout_connect(Some(Duration::from_secs(10)))
        .timeout_global(timeout)
        .timeout_recv_body(Some(Duration::from_secs(60 * 60)))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

fn get_text(url: &str) -> Result<String, String> {
    agent(Some(Duration::from_secs(15)))
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("não consegui falar com o GitHub: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("resposta inválida do GitHub: {e}"))
}

/// Pergunta ao GitHub pela última release; `None` se já está atualizado.
pub fn check() -> Result<Option<Update>, String> {
    let body = get_text(API)?;
    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    Ok(release::pick(&json, CURRENT, DOWNLOAD_PREFIX, os(), arch()))
}

/// Dá para trocar o executável no lugar? Falso em pastas do sistema e quando rodando via `cargo run`.
pub fn can_self_update() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    if exe.components().any(|c| c.as_os_str() == "target") && cfg!(debug_assertions) {
        return false;
    }
    let Some(dir) = exe.parent() else { return false };
    let probe = dir.join(format!(".arbor-write-test-{}", std::process::id()));
    let ok = std::fs::File::create(&probe).is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

pub fn cancel() {
    CANCEL.store(true, Ordering::SeqCst);
}

/// Baixa, confere e instala `update`, relatando o progresso. Bloqueia: rode numa thread.
pub fn install(update: &Update, report: &mut dyn FnMut(Progress)) -> Result<(), String> {
    CANCEL.store(false, Ordering::SeqCst);
    let sums = get_text(&update.sums_url)?;
    let expected = release::expected_hash(&sums, &update.asset_name).ok_or("a release não tem a soma SHA-256 deste binário")?;
    let dir = staging_dir()?;
    let file = dir.join(&update.asset_name);
    let result = (|| {
        let actual = download(&update.asset_url, &file, report)?;
        report(Progress::Verifying);
        if actual != expected {
            return Err("o arquivo baixado não confere com o SHA-256 da release e foi descartado".to_string());
        }
        make_executable(&file)?;
        self_replace::self_replace(&file).map_err(|e| format!("não consegui trocar o executável: {e}"))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result?;
    report(Progress::Ready);
    Ok(())
}

/// Abre a versão nova com os mesmos argumentos; quem chama encerra o processo atual em seguida.
pub fn restart() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new(exe).args(std::env::args_os().skip(1)).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Pasta própria com nome imprevisível, apagada ao fim (e as esquecidas, na próxima verificação).
fn staging_dir() -> Result<PathBuf, String> {
    let root = dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("arbor").join("updates");
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    remove_stale(&root);
    for _ in 0..8 {
        let dir = root.join(random_name());
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("não consegui criar a pasta da atualização".into())
}

fn remove_stale(root: &Path) {
    let Ok(dir) = std::fs::read_dir(root) else { return };
    for item in dir.flatten() {
        let old = item
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > Duration::from_secs(86_400));
        if old {
            let _ = std::fs::remove_dir_all(item.path());
        }
    }
}

/// 128 bits: o `RandomState` da std é semeado pela fonte aleatória do sistema.
fn random_name() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..2)
        .map(|_| {
            let mut h = std::collections::hash_map::RandomState::new().build_hasher();
            h.write_u32(std::process::id());
            format!("{:016x}", h.finish())
        })
        .collect()
}

/// Grava o corpo em `dest` calculando o SHA-256 no caminho; devolve a soma em hexadecimal.
fn download(url: &str, dest: &Path, report: &mut dyn FnMut(Progress)) -> Result<String, String> {
    let response = agent(None)
        .get(url)
        .header("Accept", "application/octet-stream")
        .call()
        .map_err(|e| format!("não consegui baixar a atualização: {e}"))?;
    let total = response.body().content_length();
    if total.is_some_and(|n| n > MAX_BYTES) {
        return Err("a atualização é maior que o limite aceito".into());
    }
    let mut body = response.into_body().into_reader();
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 128 * 1024];
    let mut received = 0u64;
    let mut last = std::time::Instant::now();
    loop {
        if CANCEL.load(Ordering::SeqCst) {
            return Err("atualização cancelada".into());
        }
        let n = body.read(&mut buf).map_err(|e| format!("o download foi interrompido: {e}"))?;
        if n == 0 {
            break;
        }
        received += n as u64;
        if received > MAX_BYTES {
            return Err("a atualização é maior que o limite aceito".into());
        }
        hasher.update(&buf[..n]);
        out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        if last.elapsed() >= Duration::from_millis(150) {
            report(Progress::Downloading { received, total });
            last = std::time::Instant::now();
        }
    }
    if total.is_some_and(|n| n != received) {
        return Err("o download terminou antes da hora".into());
    }
    out.sync_all().map_err(|e| e.to_string())?;
    report(Progress::Downloading { received, total });
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(unix)]
fn make_executable(file: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn make_executable(_file: &Path) -> Result<(), String> {
    Ok(())
}

// ---------- preferência: atualizar sozinho ----------

fn settings_file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor").join("settings"))
}

/// Ligado por padrão. `ARBOR_NO_UPDATE=1` ou a opção na paleta desligam.
pub fn auto_enabled() -> bool {
    if std::env::var_os("ARBOR_NO_UPDATE").is_some_and(|v| !v.is_empty() && v != "0") {
        return false;
    }
    let text = settings_file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    !text.lines().any(|l| l.trim() == "auto_update=false")
}

pub fn set_auto_enabled(on: bool) -> Result<(), String> {
    let file = settings_file().ok_or("sem pasta de configuração")?;
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.trim().starts_with("auto_update=")).collect();
    let line = format!("auto_update={on}");
    lines.push(&line);
    std::fs::write(&file, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn nome_do_binario_desta_plataforma() {
        let name = super::release::asset_name("1.2.3", super::os(), super::arch());
        assert!(name.starts_with("arbor-1.2.3-"));
        assert_eq!(name.ends_with(".exe"), cfg!(windows));
    }
}
