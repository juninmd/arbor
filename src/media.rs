//! Preview (Espaço) e miniaturas, gerados pelo vislo-core fora da thread da interface.

use iced::widget::image::Handle;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use tokio::sync::Semaphore;
use vislo_core::{decode, exif_info, formats, resize};

pub const THUMB_SIZE: u32 = 256;
const PREVIEW_MAX: u32 = 2048;
const TEXT_LIMIT: usize = 64 * 1024;

#[derive(Debug, Clone)]
pub enum Preview {
    #[allow(dead_code)] // largura/altura reais da imagem decodificada; usadas nos testes
    Image {
        handle: Handle,
        width: u32,
        height: u32,
        info: Vec<(String, String)>,
    },
    Text {
        content: String,
        truncated: bool,
    },
    Folder {
        dirs: usize,
        files: usize,
        bytes: u64,
        more: bool,
    },
    Other {
        info: Vec<(String, String)>,
    },
    Error(String),
}

/// Decodificações pesadas em paralelo, mas sem afogar a máquina.
static SLOTS: LazyLock<Semaphore> =
    LazyLock::new(|| Semaphore::new(std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(2, 8)));

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let _permit = SLOTS.acquire().await.ok()?;
    tokio::task::spawn_blocking(f).await.ok()
}

pub async fn preview(path: PathBuf) -> Preview {
    blocking(move || load_preview(&path)).await.unwrap_or_else(|| Preview::Error("falhou".into()))
}

/// Resultado de um pedido de miniatura.
#[derive(Debug, Clone)]
pub enum Thumb {
    Ready(Handle),
    /// Sem miniatura para este arquivo; não adianta pedir de novo.
    Failed,
    /// O arquivo saiu da janela visível antes de começar: nada foi gerado.
    Skipped,
}

/// Janela visível de cada aba. Com a roda rápida a fila enche de pedidos de linhas que já passaram; sem isto
/// a miniatura da linha onde o usuário parou esperaria atrás de todas elas.
static WINDOWS: LazyLock<Mutex<HashMap<u64, HashSet<PathBuf>>>> = LazyLock::new(Mutex::default);

/// Declara o que a aba `tab` mostra agora (substitui a janela anterior).
pub fn want(tab: u64, paths: &[PathBuf]) {
    if let Ok(mut w) = WINDOWS.lock() {
        w.insert(tab, paths.iter().cloned().collect());
    }
}

fn is_wanted(path: &Path) -> bool {
    WINDOWS.lock().map_or(true, |w| w.values().any(|set| set.contains(path)))
}

pub async fn thumbnail(path: PathBuf) -> Thumb {
    let cache = crate::thumbs::cache_dir();
    let task = blocking(move || {
        if !is_wanted(&path) {
            return Thumb::Skipped;
        }
        let _ = std::fs::create_dir_all(&cache);
        match crate::thumbs::generate(&cache, &path, THUMB_SIZE) {
            Ok(bytes) => Thumb::Ready(Handle::from_bytes(bytes)),
            Err(_) => Thumb::Failed,
        }
    });
    task.await.unwrap_or(Thumb::Failed)
}

pub fn is_image(path: &Path) -> bool {
    formats::is_image(path)
}

pub fn load_preview(path: &Path) -> Preview {
    let Ok(meta) = std::fs::metadata(path) else { return Preview::Error("não foi possível ler".into()) };
    if meta.is_dir() {
        return folder(path);
    }
    if formats::is_image(path) {
        return match image(path) {
            Ok(p) => p,
            Err(e) => Preview::Error(e),
        };
    }
    if let Some(p) = text(path) {
        return p;
    }
    let mut info = vec![("Tamanho".into(), arbor_core::human_size(meta.len()))];
    info.push(("Modificado".into(), arbor_core::human_time(meta.modified().ok())));
    Preview::Other { info }
}

fn image(path: &Path) -> Result<Preview, String> {
    let img = decode::load_fit(path, PREVIEW_MAX, PREVIEW_MAX)?;
    let img = resize::fit_owned(img, PREVIEW_MAX, PREVIEW_MAX, fast_image_resize::FilterType::CatmullRom)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let meta = exif_info::info(path);
    let mut info = vec![
        ("Formato".to_string(), meta.format.clone()),
        ("Dimensões".to_string(), format!("{} × {}", meta.width.unwrap_or(width), meta.height.unwrap_or(height))),
        ("Tamanho".to_string(), arbor_core::human_size(meta.size)),
    ];
    if let Some(c) = meta.color {
        info.push(("Cor".into(), c));
    }
    info.extend(meta.rows.into_iter().take(14).map(|r| (r.key.to_string(), r.value)));
    Ok(Preview::Image { handle: Handle::from_rgba(width, height, rgba.into_raw()), width, height, info })
}

fn text(path: &Path) -> Option<Preview> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut buf = vec![0u8; TEXT_LIMIT + 1];
    let mut n = 0;
    while n < buf.len() {
        match f.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(_) => return None,
        }
    }
    let truncated = n > TEXT_LIMIT;
    buf.truncate(n.min(TEXT_LIMIT));
    if buf.iter().take(8192).any(|&b| b == 0) {
        return None;
    }
    let content = match String::from_utf8(buf) {
        Ok(s) => s,
        // Corte no meio de um caractere: aceita perdendo a ponta.
        Err(e) if truncated && e.utf8_error().error_len().is_none() => {
            let valid = e.utf8_error().valid_up_to();
            String::from_utf8_lossy(&e.into_bytes()[..valid]).into_owned()
        }
        Err(_) => return None,
    };
    Some(Preview::Text { content: content.replace('\t', "    "), truncated })
}

fn folder(path: &Path) -> Preview {
    const LIMIT: usize = 20_000;
    let (mut dirs, mut files, mut bytes, mut more) = (0, 0, 0, false);
    if let Ok(rd) = std::fs::read_dir(path) {
        for (i, e) in rd.flatten().enumerate() {
            if i >= LIMIT {
                more = true;
                break;
            }
            match e.metadata() {
                Ok(m) if m.is_dir() => dirs += 1,
                Ok(m) => {
                    files += 1;
                    bytes += m.len();
                }
                Err(_) => {}
            }
        }
    }
    Preview::Folder { dirs, files, bytes, more }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_de_texto_e_pasta() {
        let d = tempfile::tempdir().unwrap();
        let t = d.path().join("a.rs");
        std::fs::write(&t, "fn main() {}\n").unwrap();
        assert!(matches!(load_preview(&t), Preview::Text { .. }));
        let b = d.path().join("b.bin");
        std::fs::write(&b, [0u8, 1, 2, 3]).unwrap();
        assert!(matches!(load_preview(&b), Preview::Other { .. }));
        assert!(matches!(load_preview(d.path()), Preview::Folder { files: 2, .. }));
    }

    #[test]
    fn miniatura_de_quem_saiu_da_tela_nao_e_gerada() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.png");
        image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 255, 0, 255])).save(&p).unwrap();
        let tab = 9_001;
        want(tab, std::slice::from_ref(&p));
        assert!(matches!(pollster_block(thumbnail(p.clone())), Thumb::Ready(_)), "na janela: gera");
        want(tab, &[]);
        assert!(matches!(pollster_block(thumbnail(p)), Thumb::Skipped), "fora da janela: desiste sem gerar");
    }

    fn pollster_block<T>(f: impl std::future::Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
    }

    #[test]
    fn preview_de_imagem() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("x.png");
        image::RgbaImage::from_pixel(40, 20, image::Rgba([255, 0, 0, 255])).save(&p).unwrap();
        match load_preview(&p) {
            Preview::Image { width, height, .. } => assert_eq!((width, height), (40, 20)),
            other => panic!("esperava imagem, veio {other:?}"),
        }
    }
}
