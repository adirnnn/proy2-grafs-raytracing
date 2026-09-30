//! Lectura/escritura de imágenes sin librerías externas:
//! - BMP de 24 bits (texturas y skybox; formato trivial de leer y escribir).
//! - PNG con bloques "stored" de deflate (solo para exportar capturas para el README).

use std::fs;
use std::io;
use std::path::Path;

/// Imagen RGB de 8 bits por canal, filas de arriba hacia abajo.
pub struct Rgb8 {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>, // len = width * height * 3
}

impl Rgb8 {
    pub fn new(width: usize, height: usize) -> Rgb8 {
        Rgb8 { width, height, data: vec![0; width * height * 3] }
    }
    #[inline]
    pub fn put(&mut self, x: usize, y: usize, c: [u8; 3]) {
        let i = (y * self.width + x) * 3;
        self.data[i..i + 3].copy_from_slice(&c);
    }
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }
}

fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

pub fn load_bmp(path: &Path) -> io::Result<Rgb8> {
    let b = fs::read(path)
        .map_err(|e| io::Error::new(e.kind(), format!("no se pudo leer {}: {e}", path.display())))?;
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {m}", path.display()));
    if b.len() < 54 || &b[0..2] != b"BM" {
        return Err(bad("no es un BMP"));
    }
    let offset = rd_u32(&b, 10) as usize;
    let w = rd_u32(&b, 18) as i32;
    let h = rd_u32(&b, 22) as i32;
    let bpp = rd_u16(&b, 28);
    let compression = rd_u32(&b, 30);
    if bpp != 24 || compression != 0 {
        return Err(bad("solo se soportan BMP de 24 bits sin compresión"));
    }
    let (width, height) = (w.unsigned_abs() as usize, h.unsigned_abs() as usize);
    let bottom_up = h > 0;
    let row = (width * 3 + 3) & !3;
    if b.len() < offset + row * height {
        return Err(bad("archivo truncado"));
    }
    let mut img = Rgb8::new(width, height);
    for y in 0..height {
        let src_y = if bottom_up { height - 1 - y } else { y };
        let base = offset + src_y * row;
        for x in 0..width {
            let p = base + x * 3;
            img.put(x, y, [b[p + 2], b[p + 1], b[p]]);
        }
    }
    Ok(img)
}

pub fn save_bmp(path: &Path, img: &Rgb8) -> io::Result<()> {
    let row = (img.width * 3 + 3) & !3;
    let size = 54 + row * img.height;
    let mut b = Vec::with_capacity(size);
    b.extend_from_slice(b"BM");
    b.extend_from_slice(&(size as u32).to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&54u32.to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&(img.width as i32).to_le_bytes());
    b.extend_from_slice(&(img.height as i32).to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&24u16.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&((row * img.height) as u32).to_le_bytes());
    b.extend_from_slice(&2835i32.to_le_bytes());
    b.extend_from_slice(&2835i32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    for y in (0..img.height).rev() {
        for x in 0..img.width {
            let [r, g, bl] = img.get(x, y);
            b.extend_from_slice(&[bl, g, r]);
        }
        b.resize(b.len() + row - img.width * 3, 0);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, b)
}

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc = table[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    crc ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// PNG sin compresión real (deflate "stored"). Es más grande que un PNG normal,
/// pero lo abre cualquier visor y GitHub sin depender de ninguna librería.
pub fn save_png(path: &Path, img: &Rgb8) -> io::Result<()> {
    let mut raw = Vec::with_capacity((img.width * 3 + 1) * img.height);
    for y in 0..img.height {
        raw.push(0); // filtro "None"
        raw.extend_from_slice(&img.data[y * img.width * 3..(y + 1) * img.width * 3]);
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, blk) in blocks.iter().enumerate() {
        z.push(if i + 1 == blocks.len() { 1 } else { 0 });
        let len = blk.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(blk);
    }
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(img.width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(img.height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bits, RGB
    png_chunk(&mut out, b"IHDR", &ihdr);
    png_chunk(&mut out, b"IDAT", &z);
    png_chunk(&mut out, b"IEND", &[]);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, out)
}
