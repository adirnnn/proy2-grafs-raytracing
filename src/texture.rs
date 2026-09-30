//! Texturas (cargadas desde BMP) y skybox tipo cubemap.
//!
//! Los archivos están en sRGB; al cargarlos se convierten UNA vez a espacio lineal.
//! Toda la iluminación se calcula en lineal y la corrección gamma se aplica
//! una sola vez al final (renderer::to_srgb8), así evitamos doble gamma.

use crate::image_io::load_bmp;
use crate::math::Vec3;
use std::io;
use std::path::Path;

pub struct Texture {
    pub width: usize,
    pub height: usize,
    texels: Vec<Vec3>, // lineal
}

#[inline]
pub fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

impl Texture {
    pub fn load(path: &Path) -> io::Result<Texture> {
        let img = load_bmp(path)?;
        let mut texels = Vec::with_capacity(img.width * img.height);
        for y in 0..img.height {
            for x in 0..img.width {
                let [r, g, b] = img.get(x, y);
                texels.push(Vec3::new(srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b)));
            }
        }
        Ok(Texture { width: img.width, height: img.height, texels })
    }

    #[inline]
    fn texel(&self, x: i64, y: i64, wrap: bool) -> Vec3 {
        let (w, h) = (self.width as i64, self.height as i64);
        let (x, y) = if wrap {
            (x.rem_euclid(w), y.rem_euclid(h))
        } else {
            (x.clamp(0, w - 1), y.clamp(0, h - 1))
        };
        self.texels[(y * w + x) as usize]
    }

    /// Muestreo bilineal. (u, v) con v=0 abajo; `wrap` repite la textura (bloques),
    /// `!wrap` fija los bordes (caras del skybox, para que no se filtren entre caras).
    pub fn sample(&self, u: f32, v: f32, wrap: bool) -> Vec3 {
        let fx = u * self.width as f32 - 0.5;
        let fy = (1.0 - v) * self.height as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = self.texel(x0, y0, wrap).lerp(self.texel(x0 + 1, y0, wrap), tx);
        let b = self.texel(x0, y0 + 1, wrap).lerp(self.texel(x0 + 1, y0 + 1, wrap), tx);
        a.lerp(b, ty)
    }
}

/// Orden de caras del cubemap (convención de OpenGL): +X, -X, +Y, -Y, +Z, -Z.
pub const SKY_FACES: [&str; 6] = ["px", "nx", "py", "ny", "pz", "nz"];

pub struct Skybox {
    faces: Vec<Texture>,
    pub intensity: f32,
}

/// Dirección -> (cara, u, v) con u,v en [0,1], v hacia arriba.
/// Es la inversa exacta de `face_dir` (usada por el generador de texturas).
pub fn dir_to_face(d: Vec3) -> (usize, f32, f32) {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    let (face, sc, tc, ma) = if ax >= ay && ax >= az {
        if d.x > 0.0 { (0, -d.z, -d.y, ax) } else { (1, d.z, -d.y, ax) }
    } else if ay >= az {
        if d.y > 0.0 { (2, d.x, d.z, ay) } else { (3, d.x, -d.z, ay) }
    } else if d.z > 0.0 {
        (4, d.x, -d.y, az)
    } else {
        (5, -d.x, -d.y, az)
    };
    let u = 0.5 * (sc / ma + 1.0);
    let t = 0.5 * (tc / ma + 1.0); // t crece hacia abajo en la imagen
    (face, u, 1.0 - t)
}

/// Inversa de `dir_to_face`: (cara, u, v) -> dirección (no normalizada).
pub fn face_dir(face: usize, u: f32, v: f32) -> Vec3 {
    let sc = 2.0 * u - 1.0;
    let tc = 2.0 * (1.0 - v) - 1.0;
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
    pub fn load(dir: &Path) -> io::Result<Skybox> {
        let mut faces = Vec::new();
        for name in SKY_FACES {
            faces.push(Texture::load(&dir.join(format!("{name}.bmp")))?);
        }
        Ok(Skybox { faces, intensity: 1.0 })
    }

    /// Color del entorno en la dirección `d` (espacio del mundo).
    pub fn sample(&self, d: Vec3) -> Vec3 {
        let (f, u, v) = dir_to_face(d);
        self.faces[f].sample(u, v, false) * self.intensity
    }
}
