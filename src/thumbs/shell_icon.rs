//! Ícone real do Windows para programas e atalhos (.exe, .lnk): o mesmo que o Explorer desenha, extraído
//! do recurso do executável no maior tamanho pedido e devolvido como PNG com transparência.

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    HBITMAP,
};
use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows_sys::Win32::UI::Shell::{SHDefExtractIconW, SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_ICONLOCATION, SHGFI_LARGEICON};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(Some(0)).collect()
}

/// PNG do ícone do arquivo com até `size` px.
pub fn png(path: &Path, size: u32) -> Result<Vec<u8>, String> {
    // SAFETY: inicializar COM na thread é exigido pelo SHGetFileInfoW; repetir na mesma thread só devolve S_FALSE.
    unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
    let icon = extract(path, size.min(256)).ok_or("o Windows não devolveu ícone")?;
    let rgba = to_rgba(icon);
    // SAFETY: `icon` veio de SHDefExtractIconW/SHGetFileInfoW e é nosso para destruir.
    unsafe { DestroyIcon(icon) };
    let (w, h, px) = rgba.ok_or("ícone sem bitmap colorido")?;
    let img = image::RgbaImage::from_raw(w, h, px).ok_or("bitmap do ícone com tamanho inesperado")?;
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// Onde o Explorer busca o ícone (o próprio .exe ou o alvo do atalho) e o extrai no tamanho pedido;
/// sem isso, o ícone de 32 px do sistema.
fn extract(path: &Path, size: u32) -> Option<HICON> {
    let p = wide(path.as_os_str());
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let cb = std::mem::size_of::<SHFILEINFOW>() as u32;
    // SAFETY: `p` termina em zero e `info` tem o tamanho informado.
    if unsafe { SHGetFileInfoW(p.as_ptr(), 0, &mut info, cb, SHGFI_ICONLOCATION) } != 0 {
        let len = info.szDisplayName.iter().position(|&c| c == 0).unwrap_or(0);
        let location: Vec<u16> = if len == 0 { p.clone() } else { info.szDisplayName[..len].iter().copied().chain(Some(0)).collect() };
        let mut large: HICON = std::ptr::null_mut();
        // SAFETY: `location` termina em zero; o ícone pequeno não é pedido (ponteiro nulo).
        let hr = unsafe { SHDefExtractIconW(location.as_ptr(), info.iIcon, 0, &mut large, std::ptr::null_mut(), size) };
        if hr == 0 && !large.is_null() {
            return Some(large);
        }
    }
    // SAFETY: como acima; com SHGFI_ICON o chamador destrói `hIcon`.
    let ok = unsafe { SHGetFileInfoW(p.as_ptr(), 0, &mut info, cb, SHGFI_ICON | SHGFI_LARGEICON) };
    (ok != 0 && !info.hIcon.is_null()).then_some(info.hIcon)
}

/// Pixels RGBA do ícone. Ícones antigos sem canal alfa usam a máscara para recortar o fundo.
fn to_rgba(icon: HICON) -> Option<(u32, u32, Vec<u8>)> {
    let mut ii: ICONINFO = unsafe { std::mem::zeroed() };
    // SAFETY: `icon` é válido; os bitmaps devolvidos são nossos e apagados abaixo.
    if unsafe { GetIconInfo(icon, &mut ii) } == 0 {
        return None;
    }
    let result = (|| {
        if ii.hbmColor.is_null() {
            return None;
        }
        let mut bm: BITMAP = unsafe { std::mem::zeroed() };
        // SAFETY: `bm` tem o tamanho informado.
        if unsafe { GetObjectW(ii.hbmColor, std::mem::size_of::<BITMAP>() as i32, (&mut bm as *mut BITMAP).cast()) } == 0 {
            return None;
        }
        let (w, h) = (bm.bmWidth.max(0) as u32, bm.bmHeight.max(0) as u32);
        if w == 0 || h == 0 || w > 1024 || h > 1024 {
            return None;
        }
        let mut px = bits(ii.hbmColor, w, h)?;
        if px.as_chunks::<4>().0.iter().all(|p| p[3] == 0) {
            let mask = bits(ii.hbmMask, w, h)?;
            for (p, m) in px.as_chunks_mut::<4>().0.iter_mut().zip(mask.as_chunks::<4>().0) {
                // Máscara branca é transparente; preta, opaca.
                p[3] = if m[0] == 0 { 255 } else { 0 };
            }
        }
        for p in px.as_chunks_mut::<4>().0 {
            p.swap(0, 2); // BGRA → RGBA
        }
        Some((w, h, px))
    })();
    // SAFETY: bitmaps criados por GetIconInfo; DeleteObject aceita nulo.
    unsafe {
        DeleteObject(ii.hbmColor);
        DeleteObject(ii.hbmMask);
    }
    result
}

/// Bitmap como 32 bits por pixel, de cima para baixo.
fn bits(bmp: HBITMAP, w: u32, h: u32) -> Option<Vec<u8>> {
    let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w as i32,
        biHeight: -(h as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..unsafe { std::mem::zeroed() }
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    // SAFETY: DC de memória temporário; `buf` comporta `h` linhas de `w` pixels de 4 bytes.
    let lines = unsafe {
        let dc = CreateCompatibleDC(std::ptr::null_mut());
        let n = GetDIBits(dc, bmp, 0, h, buf.as_mut_ptr().cast(), &mut bmi, DIB_RGB_COLORS);
        DeleteDC(dc);
        n
    };
    (lines == h as i32).then_some(buf)
}

#[cfg(test)]
mod tests {
    #[test]
    fn notepad_icon_is_a_transparent_png() {
        let windir = std::env::var_os("WINDIR").expect("WINDIR");
        let exe = std::path::Path::new(&windir).join("System32").join("notepad.exe");
        let png = super::png(&exe, 96).expect("ícone do bloco de notas");
        let img = image::load_from_memory(&png).expect("PNG válido").to_rgba8();
        assert!(img.width() >= 32, "pediu 96 px, veio {}", img.width());
        assert!(img.pixels().any(|p| p[3] == 0), "o canto do ícone deveria ser transparente");
        assert!(img.pixels().any(|p| p[3] == 255), "e o desenho, opaco");
    }
}
