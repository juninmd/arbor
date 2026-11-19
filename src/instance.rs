//! Instância única e "explorer padrão".
//!
//! **Instância única:** a primeira janela escuta numa porta local (127.0.0.1, porta aleatória) e grava a porta
//! e um token secreto em `instance` na pasta de configuração. Abrir o arbor de novo (atalho, Win+E,
//! "abrir pasta" de outro app) manda a pasta para essa janela, que abre uma aba nova e vem para a frente,
//! em vez de subir um segundo processo. `--new-window` ou `ARBOR_MULTI=1` forçam outra janela.
//!
//! **Explorer padrão:** no Windows grava, em HKCU (sem administrador), o arbor como ação padrão de pastas e
//! unidades; no Linux usa `xdg-mime` para `inode/directory`. Desfazer volta ao gerenciador do sistema.

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream};
use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static LISTENER: OnceLock<Mutex<Option<(TcpListener, String)>>> = OnceLock::new();

fn instance_file() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("arbor").join("instance"))
}

/// Converte o argumento da linha de comando em caminho: aceita `file:///...` (o `%U` do `.desktop`).
pub fn arg_path(arg: &str) -> PathBuf {
    match arg.strip_prefix("file://") {
        Some(rest) => {
            let rest = rest.strip_prefix("localhost").unwrap_or(rest);
            let decoded = percent_decode(rest);
            // file:///C:/x no Windows
            let decoded = if cfg!(windows) { decoded.trim_start_matches('/').to_string() } else { decoded };
            PathBuf::from(decoded)
        }
        // `"C:\"` vira `C:"` na linha de comando do Windows (a barra escapa a aspa).
        None => PathBuf::from(arg.trim_end_matches('"')),
    }
}

/// A pasta (ou arquivo) pedida na linha de comando, ignorando opções como `--new-window`.
pub fn start_arg() -> Option<PathBuf> {
    std::env::args().skip(1).find(|a| !a.starts_with("--")).map(|a| arg_path(&a))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            if let Some(b) = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn multi_requested() -> bool {
    std::env::args().any(|a| a == "--new-window") || std::env::var_os("ARBOR_MULTI").is_some_and(|v| !v.is_empty() && v != "0")
}

/// Tenta entregar `path` (ou "nova aba", se `None`) a uma janela já aberta. `true` = entregue, pode sair.
pub fn forward(path: Option<&Path>) -> bool {
    if multi_requested() {
        return false;
    }
    let Some(text) = instance_file().and_then(|f| std::fs::read_to_string(f).ok()) else { return false };
    let mut lines = text.lines();
    let (Some(port), Some(token)) = (lines.next().and_then(|p| p.trim().parse::<u16>().ok()), lines.next()) else { return false };
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(300)) else { return false };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(800)));
    let target = path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    if writeln!(stream, "{}\n{}", token.trim(), target).is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).is_ok() && reply.trim() == "ok"
}

/// Primeira janela: começa a escutar e grava porta e token. Chamado uma vez, antes da interface subir.
pub fn listen() {
    if multi_requested() {
        return;
    }
    let Ok(listener) = TcpListener::bind(("127.0.0.1", 0)) else { return };
    let Ok(addr) = listener.local_addr() else { return };
    let token = token();
    if let Some(file) = instance_file() {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if std::fs::write(&file, format!("{}\n{}\n", addr.port(), token)).is_err() {
            return;
        }
    }
    let _ = LISTENER.set(Mutex::new(Some((listener, token))));
}

/// Token imprevisível o bastante para que outro programa local não abra pastas na janela por engano.
fn token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.write_u32(std::process::id());
    let a = h.finish();
    let b = std::collections::hash_map::RandomState::new().build_hasher().finish();
    format!("{a:016x}{b:016x}")
}

/// Pedidos de outras instâncias: `Some(pasta)` ou `None` (abrir de novo sem pasta = nova aba).
pub fn requests() -> impl Stream<Item = Option<PathBuf>> {
    iced::stream::channel(8, async move |mut out: mpsc::Sender<Option<PathBuf>>| {
        let Some((listener, token)) = LISTENER.get().and_then(|m| m.lock().ok()?.take()) else { return };
        let (tx, mut rx) = mpsc::unbounded::<Option<PathBuf>>();
        std::thread::Builder::new()
            .name("arbor-instance".into())
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                    let mut reader = BufReader::new(&stream);
                    let (mut got, mut path) = (String::new(), String::new());
                    if reader.read_line(&mut got).is_err() || got.trim() != token || reader.read_line(&mut path).is_err() {
                        continue;
                    }
                    let path = path.trim();
                    let _ = (&stream).write_all(b"ok\n");
                    if tx.unbounded_send((!path.is_empty()).then(|| PathBuf::from(path))).is_err() {
                        break;
                    }
                }
            })
            .expect("thread de instância");
        use iced::futures::StreamExt;
        while let Some(req) = rx.next().await {
            if out.send(req).await.is_err() {
                break;
            }
        }
    })
}

// ---------- explorer padrão ----------

/// Liga (ou desliga) o arbor como gerenciador padrão de pastas. Devolve a mensagem para o usuário.
pub fn set_default(on: bool) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    platform_set_default(&exe, on)
}

#[cfg(windows)]
fn platform_set_default(exe: &Path, on: bool) -> Result<String, String> {
    use std::process::Command;
    let reg = |args: &[&str]| -> Result<(), String> {
        let out = Command::new("reg").args(args).output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    };
    let command = format!("\"{}\" \"%1\"", exe.display());
    // Pastas e unidades. O Win+E e as pastas especiais (Este Computador, Lixeira) continuam no Explorer:
    // trocar o "Folder" inteiro quebra itens do Painel de Controle.
    for class in ["Directory", "Drive"] {
        let shell = format!(r"HKCU\Software\Classes\{class}\shell");
        if on {
            reg(&["add", &format!(r"{shell}\arbor"), "/ve", "/d", "Abrir no arbor", "/f"])?;
            reg(&["add", &format!(r"{shell}\arbor\command"), "/ve", "/d", &command, "/f"])?;
            reg(&["add", &shell, "/ve", "/d", "arbor", "/f"])?;
        } else {
            let _ = reg(&["delete", &format!(r"{shell}\arbor"), "/f"]);
            let _ = reg(&["delete", &shell, "/ve", "/f"]);
        }
    }
    Ok(if on {
        "Pastas e unidades agora abrem no arbor (desfaça pela paleta)".into()
    } else {
        "Pastas voltaram a abrir no Explorer do Windows".into()
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_set_default(_exe: &Path, on: bool) -> Result<String, String> {
    use std::process::Command;
    let desktop = if on {
        "arbor.desktop".to_string()
    } else {
        // Volta para o primeiro gerenciador conhecido que estiver instalado.
        let candidates = ["org.gnome.Nautilus.desktop", "org.kde.dolphin.desktop", "nemo.desktop", "thunar.desktop", "pcmanfm.desktop"];
        let dirs = ["/usr/share/applications", "/usr/local/share/applications"];
        candidates
            .iter()
            .find(|c| dirs.iter().any(|d| Path::new(d).join(c).exists()))
            .map(|c| c.to_string())
            .ok_or("não achei outro gerenciador de arquivos instalado")?
    };
    let status = Command::new("xdg-mime").args(["default", &desktop, "inode/directory"]).status().map_err(|e| format!("xdg-mime: {e}"))?;
    if !status.success() {
        return Err("xdg-mime falhou (o arbor.desktop está instalado em ~/.local/share/applications?)".into());
    }
    Ok(if on { "Pastas agora abrem no arbor".into() } else { format!("Pastas voltaram a abrir em {desktop}") })
}

#[cfg(target_os = "macos")]
fn platform_set_default(_exe: &Path, _on: bool) -> Result<String, String> {
    Err("No macOS o Finder não pode ser trocado como gerenciador padrão".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argumento_file_uri() {
        if cfg!(windows) {
            assert_eq!(arg_path("file:///C:/Meus%20Docs"), PathBuf::from("C:/Meus Docs"));
        } else {
            assert_eq!(arg_path("file:///home/a/Meus%20Docs"), PathBuf::from("/home/a/Meus Docs"));
            assert_eq!(arg_path("file://localhost/tmp"), PathBuf::from("/tmp"));
        }
        assert_eq!(arg_path("/x/y"), PathBuf::from("/x/y"));
        assert_eq!(arg_path("C:\\\""), PathBuf::from("C:\\"));
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%C3%A7"), "ç");
    }
}
