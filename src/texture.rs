//! texturas (cargadas desde archivos bmp) y skybox tipo cubemap.
//!
//! los archivos vienen en srgb; al cargarlos los convertimos una sola vez a espacio
//! lineal. toda la iluminación se calcula en lineal y la corrección gamma se aplica
//! una sola vez al final (`renderer::to_srgb8`), así evitamos aplicar gamma dos veces.

use crate::image_io::load_bmp;
use crate::math::Vec3;
use std::io;
use std::path::Path;

/// imagen en memoria lista para muestrear: guarda cada texel como color lineal.
pub struct Texture {
    /// ancho en texels.
    pub width: usize,
    /// alto en texels.
    pub height: usize,
    texels: Vec<Vec3>, // lineal, fila por fila empezando por la fila de arriba
}

/// convierte un canal de color srgb de 8 bits (0 a 255) a un valor lineal entre 0 y 1.
/// es la fórmula estándar de srgb: un tramo recto para valores muy oscuros y una
/// potencia 2.4 para el resto.
#[inline]
pub fn srgb_to_linear(c: u8) -> f32 {
    // primero llevamos el byte al rango de 0 a 1.
    let c = c as f32 / 255.0;
    // por debajo de 0.04045 la curva es lineal (dividir entre 12.92); arriba es la potencia.
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

impl Texture {
    /// lee un bmp desde `path` y devuelve la textura ya convertida a lineal.
    /// devuelve error si el archivo no existe o no se puede leer.
    pub fn load(path: &Path) -> io::Result<Texture> {
        let img = load_bmp(path)?;
        // reservamos de una vez el espacio para todos los texels.
        let mut texels = Vec::with_capacity(img.width * img.height);
        for y in 0..img.height {
            for x in 0..img.width {
                // convertimos cada canal por separado de srgb a lineal.
                let [r, g, b] = img.get(x, y);
                texels.push(Vec3::new(srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b)));
            }
        }
        Ok(Texture { width: img.width, height: img.height, texels })
    }

    /// devuelve el texel en la columna `x` y la fila `y`, aunque estén fuera de la imagen.
    /// con `wrap` la textura se repite (rem_euclid da un módulo siempre positivo); sin
    /// `wrap` recortamos al borde más cercano.
    #[inline]
    fn texel(&self, x: i64, y: i64, wrap: bool) -> Vec3 {
        let (w, h) = (self.width as i64, self.height as i64);
        let (x, y) = if wrap {
            (x.rem_euclid(w), y.rem_euclid(h))
        } else {
            (x.clamp(0, w - 1), y.clamp(0, h - 1))
        };
        // los texels están guardados por filas, así que el índice es y por ancho más x.
        self.texels[(y * w + x) as usize]
    }

    /// muestreo bilineal en las coordenadas (u, v), con v = 0 abajo.
    /// `wrap` repite la textura (lo usamos en los bloques); sin `wrap` se fijan los bordes
    /// (caras del skybox, para que no se mezclen colores entre caras).
    /// devuelve el color lineal interpolado entre los cuatro texels más cercanos.
    pub fn sample(&self, u: f32, v: f32, wrap: bool) -> Vec3 {
        // pasamos (u, v) a coordenadas de texel. restamos 0.5 porque el centro de cada
        // texel está a medio texel de su esquina. v se invierte porque la fila 0 de la
        // imagen es la de arriba.
        let fx = u * self.width as f32 - 0.5;
        let fy = (1.0 - v) * self.height as f32 - 0.5;
        // texel de arriba a la izquierda y cuánto nos alejamos de él en cada eje (0 a 1).
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        // interpolamos en x en la fila y0 y en la fila y0 + 1, y luego en y entre esas dos.
        let a = self.texel(x0, y0, wrap).lerp(self.texel(x0 + 1, y0, wrap), tx);
        let b = self.texel(x0, y0 + 1, wrap).lerp(self.texel(x0 + 1, y0 + 1, wrap), tx);
        a.lerp(b, ty)
    }
}

/// nombres de archivo de las caras del cubemap en el orden de opengl:
/// x positivo, x negativo, y positivo, y negativo, z positivo, z negativo.
pub const SKY_FACES: [&str; 6] = ["px", "nx", "py", "ny", "pz", "nz"];

/// cielo de fondo formado por seis texturas, una por cara de un cubo.
pub struct Skybox {
    /// las seis caras en el orden de `SKY_FACES`.
    faces: Vec<Texture>,
    /// multiplicador del brillo del cielo (1.0 lo deja tal cual).
    pub intensity: f32,
}

/// convierte una dirección en (cara, u, v), con u y v entre 0 y 1 y v hacia arriba.
/// es la inversa exacta de `face_dir` (que usa el generador de texturas).
pub fn dir_to_face(d: Vec3) -> (usize, f32, f32) {
    // el eje con mayor valor absoluto decide en qué cara del cubo cae la dirección.
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    // para cada cara elegimos sc (horizontal), tc (vertical, crece hacia abajo) y ma
    // (el valor del eje mayor), con los signos de la convención de opengl.
    let (face, sc, tc, ma) = if ax >= ay && ax >= az {
        if d.x > 0.0 { (0, -d.z, -d.y, ax) } else { (1, d.z, -d.y, ax) }
    } else if ay >= az {
        if d.y > 0.0 { (2, d.x, d.z, ay) } else { (3, d.x, -d.z, ay) }
    } else if d.z > 0.0 {
        (4, d.x, -d.y, az)
    } else {
        (5, -d.x, -d.y, az)
    };
    // al dividir entre ma proyectamos sobre la cara (rango de menos 1 a 1) y luego
    // lo pasamos al rango de 0 a 1.
    let u = 0.5 * (sc / ma + 1.0);
    let t = 0.5 * (tc / ma + 1.0); // t crece hacia abajo en la imagen
    // nuestra v crece hacia arriba, por eso devolvemos 1 menos t.
    (face, u, 1.0 - t)
}

/// inversa de `dir_to_face`: a partir de (cara, u, v) devuelve la dirección
/// correspondiente (no normalizada; la componente del eje de la cara vale 1 en valor absoluto).
pub fn face_dir(face: usize, u: f32, v: f32) -> Vec3 {
    // regresamos u y v al rango de menos 1 a 1 (y volteamos v para obtener tc).
    let sc = 2.0 * u - 1.0;
    let tc = 2.0 * (1.0 - v) - 1.0;
    // deshacemos la elección de signos que hace `dir_to_face` para cada cara.
    match face {
        0 => Vec3::new(1.0, -tc, -sc),
        1 => Vec3::new(-1.0, -tc, sc),
        2 => Vec3::new(sc, 1.0, tc),
        3 => Vec3::new(sc, -1.0, -tc),
        4 => Vec3::new(sc, -tc, 1.0),
        _ => Vec3::new(-sc, -tc, -1.0),
    }
}

impl Skybox {
    /// carga las seis caras (px.bmp, nx.bmp, etc.) desde la carpeta `dir`.
    /// devuelve error si falta alguna cara.
    pub fn load(dir: &Path) -> io::Result<Skybox> {
        let mut faces = Vec::new();
        for name in SKY_FACES {
            faces.push(Texture::load(&dir.join(format!("{name}.bmp")))?);
        }
        Ok(Skybox { faces, intensity: 1.0 })
    }

    /// color del entorno en la dirección `d` (espacio del mundo).
    /// buscamos la cara y las coordenadas, muestreamos sin repetir y escalamos por `intensity`.
    pub fn sample(&self, d: Vec3) -> Vec3 {
        let (f, u, v) = dir_to_face(d);
        self.faces[f].sample(u, v, false) * self.intensity
    }
}
