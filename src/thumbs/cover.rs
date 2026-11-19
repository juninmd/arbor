//! Capa embutida em áudio (ID3v2 APIC do MP3, bloco PICTURE do FLAC), lida sem decodificar o áudio.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Uma capa maior que isso é lixo ou ataque: não vale a memória de uma miniatura.
const MAX_TAG: usize = 32 * 1024 * 1024;
const FRONT_COVER: u8 = 3;

/// Bytes da imagem (JPEG ou PNG) da capa, preferindo a frente quando há várias.
pub fn read(path: &Path) -> Option<Vec<u8>> {
    let mut f = File::open(path).ok()?;
    let mut magic = [0u8; 4];
    f.read_exact(&mut magic).ok()?;
    f.seek(SeekFrom::Start(0)).ok()?;
    match &magic {
        b"fLaC" => flac(&mut f),
        [b'I', b'D', b'3', _] => {
            let mut header = [0u8; 10];
            f.read_exact(&mut header).ok()?;
            let size = syncsafe(&header[6..10]);
            if size > MAX_TAG {
                return None;
            }
            let mut tag = vec![0u8; size];
            f.read_exact(&mut tag).ok()?;
            id3(header[3], header[5], &tag)
        }
        _ => None,
    }
}

fn syncsafe(b: &[u8]) -> usize {
    b.iter().fold(0, |n, &x| (n << 7) | usize::from(x & 0x7f))
}

fn be32(b: &[u8]) -> usize {
    b.iter().fold(0, |n, &x| (n << 8) | usize::from(x))
}

/// Quadros do tag ID3v2.3/2.4 (sem o cabeçalho de 10 bytes).
fn id3(version: u8, flags: u8, tag: &[u8]) -> Option<Vec<u8>> {
    // Dessincronização no tag inteiro é rara e mudaria os bytes da imagem: melhor sem capa que capa corrompida.
    if !(3..=4).contains(&version) || flags & 0x80 != 0 {
        return None;
    }
    let mut pos = 0;
    if flags & 0x40 != 0 {
        let ext = tag.get(..4)?;
        pos = if version == 4 { syncsafe(ext) } else { be32(ext) + 4 };
    }
    let mut first = None;
    while let Some(head) = tag.get(pos..pos + 10) {
        if head[0] == 0 {
            break; // preenchimento
        }
        let len = if version == 4 { syncsafe(&head[4..8]) } else { be32(&head[4..8]) };
        let body = tag.get(pos + 10..pos + 10 + len)?;
        if &head[..4] == b"APIC" {
            if let Some((kind, data)) = apic(body) {
                if kind == FRONT_COVER {
                    return Some(data.to_vec());
                }
                first.get_or_insert_with(|| data.to_vec());
            }
        }
        pos += 10 + len;
    }
    first
}

/// APIC: codificação, MIME\0, tipo, descrição terminada conforme a codificação, imagem.
fn apic(body: &[u8]) -> Option<(u8, &[u8])> {
    let (&encoding, rest) = body.split_first()?;
    let mime_end = rest.iter().position(|&b| b == 0)?;
    let (&kind, rest) = rest[mime_end + 1..].split_first()?;
    let data_start = if matches!(encoding, 1 | 2) {
        rest.as_chunks::<2>().0.iter().position(|c| *c == [0, 0])? * 2 + 2
    } else {
        rest.iter().position(|&b| b == 0)? + 1
    };
    let data = rest.get(data_start..)?;
    (!data.is_empty()).then_some((kind, data))
}

fn flac(f: &mut File) -> Option<Vec<u8>> {
    f.seek(SeekFrom::Start(4)).ok()?;
    let mut first = None;
    loop {
        let mut head = [0u8; 4];
        if f.read_exact(&mut head).is_err() {
            return first; // arquivo sem o bit de "último bloco": vale o que já foi achado
        }
        let (last, kind, len) = (head[0] & 0x80 != 0, head[0] & 0x7f, be32(&head[1..4]));
        if kind == 6 && len <= MAX_TAG {
            let mut block = vec![0u8; len];
            f.read_exact(&mut block).ok()?;
            if let Some((pic, data)) = flac_picture(&block) {
                if pic == u32::from(FRONT_COVER) {
                    return Some(data.to_vec());
                }
                first.get_or_insert_with(|| data.to_vec());
            }
        } else {
            f.seek(SeekFrom::Current(i64::try_from(len).ok()?)).ok()?;
        }
        if last {
            return first;
        }
    }
}

/// PICTURE: tipo, MIME, descrição, quatro campos de dimensão/cor, imagem; tudo com tamanho em u32 big-endian.
fn flac_picture(b: &[u8]) -> Option<(u32, &[u8])> {
    let kind = u32::try_from(be32(b.get(..4)?)).ok()?;
    let mime = be32(b.get(4..8)?);
    let desc_at = 8 + mime;
    let desc = be32(b.get(desc_at..desc_at + 4)?);
    let len_at = desc_at + 4 + desc + 16;
    let len = be32(b.get(len_at..len_at + 4)?);
    let data = b.get(len_at + 4..len_at + 4 + len)?;
    (!data.is_empty()).then_some((kind, data))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_v3(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut f = id.to_vec();
        f.extend((body.len() as u32).to_be_bytes());
        f.extend([0, 0]);
        f.extend(body);
        f
    }

    fn apic_body(encoding: u8, kind: u8, desc: &[u8], data: &[u8]) -> Vec<u8> {
        let mut b = vec![encoding];
        b.extend(b"image/png\0");
        b.push(kind);
        b.extend(desc);
        b.extend(data);
        b
    }

    fn mp3(frames: &[Vec<u8>]) -> Vec<u8> {
        let tag: Vec<u8> = frames.concat();
        let n = tag.len() + 16; // com preenchimento
        let mut out = b"ID3\x03\x00\x00".to_vec();
        out.extend([(n >> 21) as u8 & 0x7f, (n >> 14) as u8 & 0x7f, (n >> 7) as u8 & 0x7f, n as u8 & 0x7f]);
        out.extend(tag);
        out.extend([0u8; 16]);
        out.extend(b"\xff\xfb audio");
        out
    }

    fn write(name: &str, bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(name);
        std::fs::write(&p, bytes).unwrap();
        (d, p)
    }

    #[test]
    fn mp3_prefere_a_capa_da_frente() {
        let back = frame_v3(b"APIC", &apic_body(0, 4, b"verso\0", b"BACK"));
        let front = frame_v3(b"APIC", &apic_body(1, 3, b"\xff\xfeA\0\0\0", b"FRONT"));
        let title = frame_v3(b"TIT2", b"\0musica");
        let (_d, p) = write("a.mp3", &mp3(&[title, back, front]));
        assert_eq!(read(&p).as_deref(), Some(&b"FRONT"[..]));
    }

    #[test]
    fn mp3_sem_capa_ou_truncado_nao_inventa_imagem() {
        let (_d, p) = write("a.mp3", &mp3(&[frame_v3(b"TIT2", b"\0x")]));
        assert_eq!(read(&p), None);
        let mut cut = mp3(&[frame_v3(b"APIC", &apic_body(0, 3, b"\0", b"IMG"))]);
        cut.truncate(20);
        let (_d2, p2) = write("b.mp3", &cut);
        assert_eq!(read(&p2), None);
        let (_d3, p3) = write("c.mp3", b"");
        assert_eq!(read(&p3), None);
    }

    #[test]
    fn flac_le_o_bloco_picture() {
        let mut pic = 3u32.to_be_bytes().to_vec();
        pic.extend(9u32.to_be_bytes());
        pic.extend(b"image/png");
        pic.extend(0u32.to_be_bytes());
        pic.extend([0u8; 16]);
        pic.extend(4u32.to_be_bytes());
        pic.extend(b"COVR");
        let mut f = b"fLaC".to_vec();
        f.extend([0x00, 0, 0, 34]); // STREAMINFO
        f.extend([0u8; 34]);
        f.push(0x80 | 6);
        f.extend(&(pic.len() as u32).to_be_bytes()[1..]);
        f.extend(pic);
        let (_d, p) = write("a.flac", &f);
        assert_eq!(read(&p).as_deref(), Some(&b"COVR"[..]));
    }
}
