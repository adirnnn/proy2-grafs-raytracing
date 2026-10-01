//! lectura y escritura de imagenes hecha a mano, sin ninguna libreria externa.
//!
//! aqui manejamos dos formatos:
//! 1. bmp de 24 bits, que usamos para las texturas y las caras del skybox. es un formato
//!    muy simple: una cabecera fija de 54 bytes y luego los pixeles crudos en orden bgr.
//! 2. png sin compresion real (deflate con bloques "stored"), solo para exportar capturas
//!    del render para el readme. armamos los chunks, el zlib, el crc32 y el adler32 a mano.

use std::fs;
use std::io;
use std::path::Path;

/// imagen rgb con 8 bits por canal. las filas van de arriba hacia abajo, igual que en
/// pantalla, y cada pixel ocupa 3 bytes seguidos (r, g, b).
pub struct Rgb8 {
    /// ancho en pixeles.
    pub width: usize,
    /// alto en pixeles.
    pub height: usize,
    /// bytes de la imagen, su largo siempre es width * height * 3.
    pub data: Vec<u8>, // len = width * height * 3
}

impl Rgb8 {
    /// crea una imagen negra (todos los bytes en cero) del tamano pedido.
    pub fn new(width: usize, height: usize) -> Rgb8 {
        Rgb8 { width, height, data: vec![0; width * height * 3] }
    }
    /// escribe el color `c` en el pixel (x, y).
    #[inline]
    pub fn put(&mut self, x: usize, y: usize, c: [u8; 3]) {
        // el indice lineal es fila por ancho mas columna, y lo multiplicamos por 3
        // porque cada pixel ocupa tres bytes.
        let i = (y * self.width + x) * 3;
        self.data[i..i + 3].copy_from_slice(&c);
    }
    /// lee el color del pixel (x, y) como un arreglo [r, g, b].
    #[inline]
    pub fn get(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 3;
        [self.data[i], self.data[i + 1], self.data[i + 2]]
    }
}

/// lee un entero de 16 bits en little endian (byte menos significativo primero),
/// que es como guarda los numeros el formato bmp.
fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
/// lee un entero de 32 bits en little endian a partir de la posicion `o`.
fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// carga un bmp de 24 bits sin compresion y lo devuelve como `Rgb8` con las filas
/// ya puestas de arriba hacia abajo y los canales en orden rgb.
pub fn load_bmp(path: &Path) -> io::Result<Rgb8> {
    // leemos todo el archivo a memoria; si falla agregamos la ruta al mensaje de error
    // para saber cual textura falta.
    let b = fs::read(path)
        .map_err(|e| io::Error::new(e.kind(), format!("no se pudo leer {}: {e}", path.display())))?;
    // pequena funcion para fabricar errores de datos invalidos con la ruta incluida.
    let bad = |m: &str| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {m}", path.display()));
    // la cabecera completa mide 54 bytes (14 del encabezado de archivo y 40 del de info)
    // y todo bmp empieza con las letras `BM`.
    if b.len() < 54 || &b[0..2] != b"BM" {
        return Err(bad("no es un BMP"));
    }
    // en el byte 10 esta el desplazamiento donde empiezan los pixeles.
    let offset = rd_u32(&b, 10) as usize;
    // en los bytes 18 y 22 estan el ancho y el alto; son enteros con signo, por eso
    // los pasamos a i32 (un alto negativo significa que las filas van de arriba abajo).
    let w = rd_u32(&b, 18) as i32;
    let h = rd_u32(&b, 22) as i32;
    // en el byte 28 estan los bits por pixel y en el 30 el tipo de compresion.
    let bpp = rd_u16(&b, 28);
    let compression = rd_u32(&b, 30);
    // solo aceptamos el caso mas sencillo: 24 bits por pixel y compresion 0 (ninguna).
    if bpp != 24 || compression != 0 {
        return Err(bad("solo se soportan BMP de 24 bits sin compresión"));
    }
    // nos quedamos con el valor absoluto de las medidas y recordamos el sentido de las filas.
    let (width, height) = (w.unsigned_abs() as usize, h.unsigned_abs() as usize);
    // si el alto es positivo el bmp esta guardado de abajo hacia arriba (lo normal).
    let bottom_up = h > 0;
    // cada fila del bmp se rellena con ceros hasta un multiplo de 4 bytes. sumar 3 y
    // borrar los dos bits bajos con `& !3` redondea hacia arriba al multiplo de 4.
    let row = (width * 3 + 3) & !3;
    // revisamos que el archivo tenga todos los bytes que la cabecera promete.
    if b.len() < offset + row * height {
        return Err(bad("archivo truncado"));
    }
    let mut img = Rgb8::new(width, height);
    for y in 0..height {
        // si las filas vienen de abajo hacia arriba, la fila y de nuestra imagen es la
        // fila height menos 1 menos y del archivo; asi la imagen queda derecha.
        let src_y = if bottom_up { height - 1 - y } else { y };
        let base = offset + src_y * row;
        for x in 0..width {
            let p = base + x * 3;
            // el bmp guarda los canales al reves (azul, verde, rojo), asi que los
            // volteamos para dejarlos en rgb.
            img.put(x, y, [b[p + 2], b[p + 1], b[p]]);
        }
    }
    Ok(img)
}

/// guarda la imagen como bmp de 24 bits, con las filas de abajo hacia arriba como
/// manda el formato. crea la carpeta de destino si todavia no existe.
pub fn save_bmp(path: &Path, img: &Rgb8) -> io::Result<()> {
    // largo de cada fila con su relleno hasta multiplo de 4 bytes.
    let row = (img.width * 3 + 3) & !3;
    // tamano total del archivo: 54 bytes de cabecera mas todas las filas.
    let size = 54 + row * img.height;
    let mut b = Vec::with_capacity(size);
    // aqui escribimos la cabecera de archivo (14 bytes):
    // la firma `BM` que identifica al bmp.
    b.extend_from_slice(b"BM");
    // el tamano total del archivo en bytes.
    b.extend_from_slice(&(size as u32).to_le_bytes());
    // cuatro bytes reservados que siempre van en cero.
    b.extend_from_slice(&0u32.to_le_bytes());
    // el desplazamiento hasta los pixeles: justo despues de los 54 bytes de cabecera.
    b.extend_from_slice(&54u32.to_le_bytes());
    // ahora la cabecera de informacion (40 bytes), empezando por su propio tamano.
    b.extend_from_slice(&40u32.to_le_bytes());
    // ancho y alto; el alto va positivo para indicar filas de abajo hacia arriba.
    b.extend_from_slice(&(img.width as i32).to_le_bytes());
    b.extend_from_slice(&(img.height as i32).to_le_bytes());
    // numero de planos de color, que siempre es 1.
    b.extend_from_slice(&1u16.to_le_bytes());
    // bits por pixel: 24, o sea un byte por canal.
    b.extend_from_slice(&24u16.to_le_bytes());
    // compresion 0, los pixeles van crudos.
    b.extend_from_slice(&0u32.to_le_bytes());
    // tamano de los datos de pixeles, contando el relleno de cada fila.
    b.extend_from_slice(&((row * img.height) as u32).to_le_bytes());
    // resolucion horizontal y vertical en pixeles por metro; 2835 equivale a 72 dpi.
    b.extend_from_slice(&2835i32.to_le_bytes());
    b.extend_from_slice(&2835i32.to_le_bytes());
    // colores de paleta usados e importantes: 0 porque no usamos paleta.
    b.extend_from_slice(&0u32.to_le_bytes());
    b.extend_from_slice(&0u32.to_le_bytes());
    // recorremos las filas al reves para que la de abajo quede primero en el archivo.
    for y in (0..img.height).rev() {
        for x in 0..img.width {
            // cada pixel se escribe en orden bgr.
            let [r, g, bl] = img.get(x, y);
            b.extend_from_slice(&[bl, g, r]);
        }
        // agregamos los ceros de relleno que faltan para llegar al multiplo de 4.
        b.resize(b.len() + row - img.width * 3, 0);
    }
    // nos aseguramos de que exista la carpeta antes de escribir.
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, b)
}

/// calcula el crc32 (el mismo de zip y png) que protege cada chunk del png.
fn crc32(data: &[u8]) -> u32 {
    // primero armamos la tabla de 256 entradas: el crc de cada byte posible. usamos el
    // polinomio en forma reflejada `0xEDB8_8320` porque el png procesa los bits del menos
    // significativo al mas significativo.
    let mut table = [0u32; 256];
    for (i, t) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        // ocho pasos de division polinomial, uno por bit: si el bit bajo es 1 corremos
        // y aplicamos xor con el polinomio, si no solo corremos.
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    // el crc empieza con todos los bits en 1.
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        // con la tabla procesamos un byte entero por paso: el byte bajo del crc
        // combinado con el dato elige la entrada y el resto se corre 8 bits.
        crc = table[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    // al final se invierten todos los bits, asi lo pide el estandar.
    crc ^ 0xFFFF_FFFF
}

/// calcula el adler32, la suma de verificacion que va al final del flujo zlib.
fn adler32(data: &[u8]) -> u32 {
    // `a` es 1 mas la suma de todos los bytes y `b` es la suma acumulada de los `a`.
    let (mut a, mut b) = (1u32, 0u32);
    // 5552 es la mayor cantidad de bytes que podemos sumar sin que `b` se desborde en
    // u32, por eso sacamos el modulo solo cada 5552 bytes y no en cada byte.
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += x as u32;
            b += a;
        }
        // 65521 es el primo mas grande menor que 65536.
        a %= 65521;
        b %= 65521;
    }
    // el resultado junta las dos sumas: `b` en los 16 bits altos y `a` en los bajos.
    (b << 16) | a
}

/// agrega un chunk de png a `out`. cada chunk tiene: largo de los datos (4 bytes big
/// endian), tipo de 4 letras, los datos, y el crc32 del tipo junto con los datos.
fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
    // el largo cuenta solo los datos, no el tipo ni el crc.
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    // guardamos donde empieza el tipo porque el crc se calcula desde ahi.
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    // el crc cubre tipo y datos, pero no el campo de largo.
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// guarda la imagen como png sin compresion real (deflate con bloques "stored").
/// el archivo sale mas grande que un png normal, pero cualquier visor y github lo abren
/// y no dependemos de ninguna libreria.
pub fn save_png(path: &Path, img: &Rgb8) -> io::Result<()> {
    // primero armamos los datos crudos: cada fila lleva delante un byte con el tipo de
    // filtro y luego sus pixeles rgb.
    let mut raw = Vec::with_capacity((img.width * 3 + 1) * img.height);
    for y in 0..img.height {
        raw.push(0); // filtro 0, o sea "none": la fila va tal cual sin predecir nada
        raw.extend_from_slice(&img.data[y * img.width * 3..(y + 1) * img.width * 3]);
    }
    // cabecera zlib de dos bytes: 0x78 dice metodo deflate con ventana de 32 kb, y 0x01
    // completa el chequeo (0x7801 es multiplo de 31) sin diccionario y nivel rapido.
    let mut z = vec![0x78, 0x01];
    // un bloque stored puede llevar como maximo 65535 bytes, asi que partimos los datos.
    let blocks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, blk) in blocks.iter().enumerate() {
        // byte de cabecera del bloque: el bit bajo es bfinal (1 solo en el ultimo bloque)
        // y los dos bits siguientes en 00 indican tipo stored; el resto es relleno.
        z.push(if i + 1 == blocks.len() { 1 } else { 0 });
        // luego van len y nlen en little endian; nlen es len con todos los bits
        // invertidos y sirve para que el lector verifique el largo.
        let len = blk.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        // y despues los bytes del bloque copiados tal cual.
        z.extend_from_slice(blk);
    }
    // el flujo zlib termina con el adler32 de los datos sin comprimir, en big endian.
    z.extend_from_slice(&adler32(&raw).to_be_bytes());

    // la firma de 8 bytes con la que empieza todo png.
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    // el chunk `IHDR` describe la imagen: ancho y alto en big endian y luego cinco bytes.
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(img.width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(img.height as u32).to_be_bytes());
    // profundidad 8 bits, tipo de color 2 (rgb), compresion 0, filtro 0 y sin entrelazado.
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bits por canal, color rgb
    png_chunk(&mut out, b"IHDR", &ihdr);
    // el chunk `IDAT` lleva el flujo zlib completo con los pixeles.
    png_chunk(&mut out, b"IDAT", &z);
    // el chunk `IEND` va vacio y marca el final del archivo.
    png_chunk(&mut out, b"IEND", &[]);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, out)
}
