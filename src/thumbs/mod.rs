//! Miniaturas: imagens pelo vislo-core; capa de áudio, vídeo e PDF extraídos aqui e guardados no mesmo
//! cache em disco, com chave por caminho + tamanho + data, então editar o arquivo invalida a miniatura.

mod cover;
mod external;
#[cfg(windows)]
mod shell_icon;

use fast_image_resize::FilterType;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;
use vislo_core::{formats, resize, thumbs};

/// Teto do cache em disco; ao abrir, os mais antigos saem até sobrar 80% disso.
const CACHE_MAX_BYTES: u64 = 512 * 1024 * 1024;

const COVER: &[&str] = &["mp3", "flac"];
/// Sem "ts": quase sempre é TypeScript, e cada um custaria um ffmpeg à toa.
const VIDEO: &[&str] = &["mp4", "mkv", "mov", "avi", "webm", "wmv", "flv", "m4v", "mpg", "mpeg", "mts", "m2ts", "3gp"];
const PDF: &[&str] = &["pdf"];
/// Programas e atalhos: no Windows, o ícone real do Explorer em vez do ícone genérico.
const PROGRAM: &[&str] = &["exe", "lnk", "msi", "url"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Image,
    Cover,
    Video,
    Pdf,
    Program,
}

fn source(path: &Path) -> Option<Source> {
    if formats::is_image(path) {
        Some(Source::Image)
    } else if formats::has_ext(path, COVER) {
        Some(Source::Cover)
    } else if formats::has_ext(path, VIDEO) {
        Some(Source::Video)
    } else if formats::has_ext(path, PDF) {
        Some(Source::Pdf)
    } else if program(path) {
        Some(Source::Program)
    } else {
        None
    }
}

pub fn supported(path: &Path) -> bool {
    source(path).is_some()
}

/// Ícone de programa: desenhado inteiro (sem cortar) e sem fundo, como no Explorer.
pub fn program(path: &Path) -> bool {
    cfg!(windows) && formats::has_ext(path, PROGRAM)
}

pub fn cache_dir() -> PathBuf {
    dirs::cache_dir().unwrap_or_else(std::env::temp_dir).join("arbor").join("thumbs")
}

/// Mantém o cache limitado; roda fora da interface porque varre a pasta inteira.
pub fn prune_cache() {
    std::thread::spawn(|| {
        if let Err(e) = thumbs::evict(&cache_dir(), CACHE_MAX_BYTES) {
            if e.kind() != std::io::ErrorKind::NotFound {
                eprintln!("arbor: limpeza do cache de miniaturas falhou: {e}");
            }
        }
    });
}

/// JPEG com o lado maior até `size`.
pub fn generate(cache: &Path, path: &Path, size: u32) -> Result<Vec<u8>, String> {
    let kind = source(path).ok_or("sem miniatura para este tipo")?;
    if kind == Source::Image {
        return thumbs::thumbnail(cache, path, size);
    }
    let size = size.clamp(thumbs::MIN_SIZE, thumbs::MAX_SIZE);
    #[cfg(windows)]
    if kind == Source::Program {
        // PNG para manter a transparência; o ícone já sai no tamanho certo, sem redimensionar.
        let file = cache_file(cache, path, size, "png")?;
        if let Some(hit) = std::fs::read(&file).ok().filter(|b| b.ends_with(PNG_END)) {
            return Ok(hit);
        }
        let png = shell_icon::png(path, size)?;
        if let Err(e) = write_atomic(&file, &png) {
            eprintln!("arbor: gravar ícone no cache falhou: {e}");
        }
        return Ok(png);
    }
    let file = cache_file(cache, path, size, "jpg")?;
    if let Some(hit) = std::fs::read(&file).ok().filter(|b| is_complete_jpeg(b)) {
        return Ok(hit);
    }
    let encoded = match kind {
        Source::Cover => cover::read(path).ok_or("sem capa embutida")?,
        Source::Video => external::video_frame(path)?,
        Source::Pdf => external::pdf_page(path)?,
        Source::Image | Source::Program => unreachable!("tratado acima ou só existe no Windows"),
    };
    let img = image::load_from_memory(&encoded).map_err(|e| e.to_string())?;
    let jpeg = resize::to_jpeg(&resize::fit_owned(img, size, size, FilterType::CatmullRom)?, 80)?;
    // O cache é otimização: falhar ao gravar não pode falhar a miniatura.
    if let Err(e) = write_atomic(&file, &jpeg) {
        eprintln!("arbor: gravar miniatura no cache falhou: {e}");
    }
    Ok(jpeg)
}

/// Fim do bloco IEND: um PNG cortado no meio da gravação não termina assim.
#[cfg(windows)]
const PNG_END: &[u8] = &[0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82];

fn cache_file(dir: &Path, src: &Path, size: u32, ext: &str) -> Result<PathBuf, String> {
    let meta = std::fs::metadata(src).map_err(|e| e.to_string())?;
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_nanos());
    Ok(dir.join(format!("{}.{ext}", thumbs::cache_key(src, meta.len(), mtime, size))))
}

/// Um arquivo cortado por queda ou disco cheio não pode ser servido como miniatura.
fn is_complete_jpeg(b: &[u8]) -> bool {
    b.len() >= 4 && b.starts_with(&[0xFF, 0xD8, 0xFF]) && b.ends_with(&[0xFF, 0xD9])
}

fn write_atomic(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = file.with_extension(format!("tmp{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    let result = std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, file));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// PDF mínimo de uma página com um quadrado vermelho.
    pub const TINY_PDF: &[u8] = b"%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 100]/Contents 4 0 R>>endobj\n\
4 0 obj<</Length 25>>stream\n1 0 0 rg 0 0 100 100 re f\nendstream endobj\n\
trailer<</Root 1 0 R>>\n%%EOF\n";

    #[test]
    fn classifica_por_extensao_sem_diferenciar_caixa() {
        assert_eq!(source(Path::new("a.PNG")), Some(Source::Image));
        assert_eq!(source(Path::new("a.psd")), Some(Source::Image));
        assert_eq!(source(Path::new("a.Mp3")), Some(Source::Cover));
        assert_eq!(source(Path::new("a.mkv")), Some(Source::Video));
        assert_eq!(source(Path::new("a.pdf")), Some(Source::Pdf));
        assert_eq!(source(Path::new("app.ts")), None, "TypeScript não é vídeo");
        assert_eq!(source(Path::new("sem_extensao")), None);
    }

    #[test]
    fn capa_vira_jpeg_no_cache_e_editar_invalida() {
        let d = tempfile::tempdir().unwrap();
        let cache = d.path().join("cache");
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(600, 300, image::Rgba([0, 0, 255, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let mut tag = b"APIC".to_vec();
        let body = [&b"\0image/png\0\x03\0"[..], &png].concat();
        tag.extend((body.len() as u32).to_be_bytes());
        tag.extend([0, 0]);
        tag.extend(body);
        let n = tag.len();
        let mut mp3 = b"ID3\x03\x00\x00".to_vec();
        mp3.extend([(n >> 21) as u8 & 0x7f, (n >> 14) as u8 & 0x7f, (n >> 7) as u8 & 0x7f, n as u8 & 0x7f]);
        mp3.extend(tag);
        let song = d.path().join("faixa.mp3");
        std::fs::write(&song, &mp3).unwrap();

        let jpeg = generate(&cache, &song, 256).unwrap();
        let img = image::load_from_memory(&jpeg).unwrap();
        assert_eq!((img.width(), img.height()), (256, 128), "proporção mantida, lado maior = 256");
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 1);
        assert_eq!(generate(&cache, &song, 256).unwrap(), jpeg, "segunda vez vem do cache");

        std::fs::write(&song, [&mp3[..], b"mais audio"].concat()).unwrap();
        generate(&cache, &song, 256).unwrap();
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 2, "arquivo editado gera nova chave");
    }

    #[test]
    fn sem_capa_e_tipo_desconhecido_sao_erro() {
        let d = tempfile::tempdir().unwrap();
        let song = d.path().join("vazio.mp3");
        std::fs::write(&song, b"\xff\xfb").unwrap();
        assert!(generate(d.path(), &song, 256).is_err());
        assert!(generate(d.path(), &d.path().join("x.txt"), 256).is_err());
    }
}
