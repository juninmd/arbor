//! Vídeo e PDF por programas externos opcionais: sem eles, o arquivo fica só com o ícone do tipo.
//! Nada passa por shell; o caminho vai como argumento único e o processo morre se passar do prazo.

use std::ffi::OsString;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_OUTPUT: u64 = 16 * 1024 * 1024;
/// Gera maior que a miniatura para o redimensionamento final ficar nítido.
const RENDER: u32 = 512;

/// Um quadro do vídeo (JPEG). Tenta 1 s adentro para fugir da tela preta; vídeo curto cai no começo.
pub fn video_frame(path: &Path) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "macos")]
    if let Ok(png) = quicklook(path) {
        return Ok(png);
    }
    ffmpeg(path, Some("1")).or_else(|_| ffmpeg(path, None))
}

/// Primeira página do PDF (PNG).
pub fn pdf_page(path: &Path) -> Result<Vec<u8>, String> {
    #[cfg(target_os = "macos")]
    if let Ok(png) = quicklook(path) {
        return Ok(png);
    }
    let mut cmd = Command::new("pdftoppm");
    let size = RENDER.to_string();
    cmd.args(["-png", "-singlefile", "-f", "1", "-l", "1", "-scale-to", &size, "--"]).arg(path);
    run(cmd, TIMEOUT)
}

fn ffmpeg(path: &Path, seek: Option<&str>) -> Result<Vec<u8>, String> {
    // "file:" impede que um nome como "concat:x" ou "http:x" seja lido como protocolo do ffmpeg.
    let mut input = OsString::from("file:");
    input.push(path);
    let scale = format!("scale={RENDER}:{RENDER}:force_original_aspect_ratio=decrease");
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-nostdin", "-v", "error", "-threads", "1"]);
    if let Some(s) = seek {
        cmd.args(["-ss", s]);
    }
    cmd.arg("-i").arg(input).args(["-frames:v", "1", "-vf", &scale, "-f", "image2pipe", "-c:v", "mjpeg", "-"]);
    run(cmd, TIMEOUT)
}

/// Quick Look gera a miniatura que o Finder mostraria, sem nada instalado.
#[cfg(target_os = "macos")]
fn quicklook(path: &Path) -> Result<Vec<u8>, String> {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let out = std::env::temp_dir().join(format!("arbor-ql-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut cmd = Command::new("qlmanage");
    cmd.args(["-t", "-s", &RENDER.to_string(), "-o"]).arg(&out).arg(path);
    let result = run(cmd, TIMEOUT).and_then(|_| {
        let name = format!("{}.png", path.file_name().unwrap_or_default().to_string_lossy());
        std::fs::read(out.join(name)).map_err(|e| e.to_string())
    });
    let _ = std::fs::remove_dir_all(&out);
    result
}

/// Saída padrão do programa, ou erro se ele faltar, falhar, não escrever nada ou estourar o prazo.
fn run(mut cmd: Command, timeout: Duration) -> Result<Vec<u8>, String> {
    let name = cmd.get_program().to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| format!("{name}: {e}"))?;
    let stdout = child.stdout.take().ok_or_else(|| format!("{name}: sem saída"))?;
    // Lido em paralelo: um pipe cheio travaria o processo antes de ele terminar.
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        stdout.take(MAX_OUTPUT).read_to_end(&mut buf).map(|_| buf)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("{name}: {e}"))? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{name}: passou de {}s", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let out = reader.join().map_err(|_| format!("{name}: leitura falhou"))?.map_err(|e| format!("{name}: {e}"))?;
    if !status.success() || out.is_empty() {
        return Err(format!("{name}: saiu com {status}"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has(tool: &str) -> bool {
        Command::new(tool).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()
    }

    #[test]
    fn programa_ausente_vira_erro_e_nao_panico() {
        assert!(run(Command::new("arbor-programa-que-nao-existe"), TIMEOUT).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn processo_lento_morre_no_prazo_e_saida_vazia_e_erro() {
        let mut slow = Command::new("sh");
        slow.args(["-c", "exec sleep 60"]);
        let start = Instant::now();
        assert!(run(slow, Duration::from_millis(300)).is_err());
        assert!(start.elapsed() < Duration::from_secs(5), "não pode esperar o sleep inteiro");
        assert!(run(Command::new("true"), TIMEOUT).is_err(), "sucesso sem bytes não é miniatura");
    }

    #[test]
    fn quadro_de_video_com_ffmpeg() {
        if !has("ffmpeg") {
            eprintln!("ffmpeg ausente: teste de vídeo não roda nesta máquina");
            return;
        }
        let d = tempfile::tempdir().unwrap();
        // Nome com cara de protocolo: tem que ser lido como arquivo.
        let p = d.path().join("concat:clip.mp4");
        let ok = Command::new("ffmpeg")
            .args(["-v", "error", "-f", "lavfi", "-i", "color=red:s=64x48:d=0.5", "-pix_fmt", "yuv420p"])
            .arg(&p)
            .status()
            .unwrap();
        assert!(ok.success());
        let jpeg = video_frame(&p).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8]), "esperava JPEG");
    }

    #[test]
    fn primeira_pagina_de_pdf_com_pdftoppm() {
        if !has("pdftoppm") {
            eprintln!("pdftoppm ausente: teste de PDF não roda nesta máquina");
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("relatório final.pdf");
        std::fs::write(&p, crate::thumbs::tests::TINY_PDF).unwrap();
        let png = pdf_page(&p).unwrap();
        assert!(png.starts_with(b"\x89PNG"), "esperava PNG");
    }
}
