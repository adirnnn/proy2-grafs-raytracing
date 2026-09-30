//! Construcción del diorama "Las Noches bajo la Luna Eterna".
//!
//! Unidades: 1 unidad = 1 bloque. Y hacia arriba. El diorama ocupa x,z en [-13, 13]
//! sobre un zócalo de basalto; la cara superior del zócalo está en y = 0.

use crate::bvh::Bvh;
use crate::geometry::Object;
use crate::material::*;
use crate::math::{hash_u32, Mat3, Vec3};
use crate::texture::{Skybox, Texture};
use std::f32::consts::PI;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug)]
pub enum Light {
    /// `dir` apunta HACIA la luz. `spread` = tamaño angular aproximado (sombras suaves).
    Directional { dir: Vec3, color: Vec3, spread: f32 },
    /// Luz puntual con caída suave a partir de `radius`.
    Point { pos: Vec3, color: Vec3, radius: f32 },
}

pub struct Scene {
    pub objects: Vec<Object>,
    pub bvh: Bvh,
    pub materials: Vec<Material>,
    /// Una textura por material (mismo índice).
    pub textures: Vec<Texture>,
    pub skybox: Skybox,
    pub lights: Vec<Light>,
    pub ambient_sky: Vec3,
    pub ambient_ground: Vec3,
}

/// Dirección (en el mundo) hacia la luna dibujada en el skybox.
/// El generador de texturas usa la misma constante para pintar la luna ahí.
pub const MOON_DIR: Vec3 = Vec3::new(-0.14, 0.14, -0.98);
/// Dirección de la Garganta (grieta roja) en el cielo: detrás de la cámara.
pub const GARGANTA_DIR: Vec3 = Vec3::new(0.18, 0.24, 0.95);

impl Scene {
    /// Luz ambiente hemisférica: cielo nocturno arriba, rebote de la arena abajo.
    #[inline]
    pub fn ambient(&self, n: Vec3) -> Vec3 {
        self.ambient_ground.lerp(self.ambient_sky, 0.5 + 0.5 * n.y)
    }

    pub fn load(assets: &Path) -> io::Result<Scene> {
        let materials: Vec<Material> = MATERIALS.to_vec();
        let mut textures = Vec::new();
        for m in &materials {
            textures.push(Texture::load(&assets.join("textures").join(format!("{}.bmp", m.texture)))?);
        }
        let skybox = Skybox::load(&assets.join("skybox"))?;
        let objects = build_diorama();
        let bvh = Bvh::build(&objects);
        let lights = vec![
            // Luna: luz principal fría, con sombras suaves.
            Light::Directional {
                dir: MOON_DIR.normalized(),
                color: Vec3::new(0.78, 0.86, 1.05) * 3.0,
                spread: 0.025,
            },
            // Relleno tenue desde el lado del espectador (brillo del cielo estrellado).
            Light::Directional {
                dir: Vec3::new(0.55, 0.65, 0.55).normalized(),
                color: Vec3::new(0.10, 0.12, 0.22),
                spread: 0.0,
            },
            // Luz del "cielo falso" dentro de la cúpula: se escapa por el corte.
            Light::Point { pos: DOME_C + Vec3::new(2.2, 3.0, 2.6), color: Vec3::new(1.4, 1.8, 2.4), radius: 5.0 },
        ];
        Ok(Scene {
            objects,
            bvh,
            materials,
            textures,
            skybox,
            lights,
            ambient_sky: Vec3::new(0.022, 0.026, 0.060),
            ambient_ground: Vec3::new(0.030, 0.028, 0.034),
        })
    }
}

// ----------------------------------------------------------------------------
// Diseño del diorama
// ----------------------------------------------------------------------------

pub const HALF: i32 = 12; // celdas de -12..=12 (25x25 bloques)
pub const DOME_C: Vec3 = Vec3::new(-3.0, 1.0, -5.0);
const DOME_R: f32 = 7.5;

fn noise2(x: f32, z: f32, seed: u32) -> f32 {
    // Ruido de valor suave (para dunas).
    let (xi, zi) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - xi as f32, z - zi as f32);
    let h = |a: i32, b: i32| {
        (hash_u32((a as u32).wrapping_mul(73_856_093) ^ (b as u32).wrapping_mul(19_349_663) ^ seed) & 0xFFFF) as f32
            / 65535.0
    };
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let a = h(xi, zi) + (h(xi + 1, zi) - h(xi, zi)) * sx;
    let b = h(xi, zi + 1) + (h(xi + 1, zi + 1) - h(xi, zi + 1)) * sx;
    a + (b - a) * sz
}

/// Altura de la arena (en bloques) en la celda (i, j).
pub fn sand_height(i: i32, j: i32) -> i32 {
    let (x, z) = (i as f32, j as f32);
    let n = noise2(x * 0.16, z * 0.16, 7) * 0.7 + noise2(x * 0.33, z * 0.33, 13) * 0.3;
    // Dunas más altas en los bordes, planas cerca de la plaza.
    let edge = ((x.abs().max(z.abs())) / 12.0).powi(2);
    (1.0 + n * 2.6 + edge * 0.6).floor() as i32
}

fn in_plaza(i: i32, j: i32) -> bool {
    (-3..=4).contains(&i) && (1..=6).contains(&j)
}

fn in_fortress(i: i32, j: i32) -> bool {
    let d = ((i as f32 - DOME_C.x).powi(2) + (j as f32 - DOME_C.z).powi(2)).sqrt();
    d <= DOME_R + 0.6 || (j <= -8 && (-8..=3).contains(&i))
}

fn cube_at(x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, m: u16) -> Object {
    Object::cube(Vec3::new(x, y, z), Vec3::new(x + sx, y + sy, z + sz), m)
}

/// Bloque unitario en la celda (i, y, j).
fn block(i: i32, y: i32, j: i32, m: u16) -> Object {
    cube_at(i as f32 - 0.5, y as f32, j as f32 - 0.5, 1.0, 1.0, 1.0, m)
}

pub fn build_diorama() -> Vec<Object> {
    let mut o = Vec::new();

    // Zócalo de basalto (la "caja" del diorama).
    o.push(cube_at(-13.2, -3.0, -13.2, 26.4, 3.0, 26.4, BASALT));

    // Terreno: una columna por celda (un solo cubo alto por columna, UV por bloque).
    for i in -HALF..=HALF {
        for j in -HALF..=HALF {
            let (x0, z0) = (i as f32 - 0.5, j as f32 - 0.5);
            if in_plaza(i, j) {
                // Plaza de obsidiana pulida, con borde de piedra.
                let border = i == -3 || i == 4 || j == 1 || j == 6;
                let m = if border { STONE } else { OBSIDIAN };
                o.push(cube_at(x0, 0.0, z0, 1.0, if border { 1.25 } else { 1.0 }, 1.0, m));
            } else if in_fortress(i, j) {
                o.push(cube_at(x0, 0.0, z0, 1.0, 1.0, 1.0, STONE));
            } else {
                let h = sand_height(i, j);
                o.push(cube_at(x0, 0.0, z0, 1.0, h as f32, 1.0, SAND));
            }
        }
    }

    build_dome(&mut o);
    build_towers(&mut o);
    build_zangetsu(&mut o, Vec3::new(6.6, 0.0, 2.0));
    // Bosque de cuarzo: dos grupos que enmarcan la plaza, la cúpula y la torre.
    build_quartz_tree(&mut o, Vec3::new(-8.0, 0.0, 7.0), 1, 1.35);
    build_quartz_tree(&mut o, Vec3::new(-10.5, 0.0, -0.5), 5, 1.0);
    build_quartz_tree(&mut o, Vec3::new(-5.0, 0.0, 10.5), 2, 0.9);
    build_quartz_tree(&mut o, Vec3::new(11.0, 0.0, 4.0), 3, 1.5);
    build_quartz_tree(&mut o, Vec3::new(10.0, 0.0, -3.0), 4, 1.2);
    build_quartz_tree(&mut o, Vec3::new(1.5, 0.0, 11.0), 6, 0.8);
    // Cristales pequeños en el suelo.
    for (k, (i, j)) in [(-6, 3), (2, 9), (7, 6), (-11, 9), (11, 2), (-2, 11), (6, -6)].iter().enumerate() {
        build_shard(&mut o, *i, *j, k as u32);
    }
    build_hollow_mask(&mut o, Vec3::new(-9.0, 0.0, 10.0));
    build_ruins(&mut o);
    o
}

/// Cúpula de Las Noches: cascarón de bloques con un corte hacia el espectador.
/// Cara exterior de piedra, cara interior "cielo falso" (material alterno).
fn build_dome(o: &mut Vec<Object>) {
    let c = DOME_C;
    let r = DOME_R as i32 + 1;
    for dx in -r..=r {
        for dz in -r..=r {
            for dy in 0..=r {
                let p = Vec3::new(dx as f32, dy as f32 + 0.5, dz as f32);
                let d = p.length();
                // Fuera del cascarón, o donde la torre central atraviesa la cúpula.
                if d < DOME_R - 1.0 || d > DOME_R || (dx.abs() <= 1 && dz.abs() <= 1) {
                    continue;
                }
                // Corte: una "cuña" abierta hacia +z/+x, dejando un arco arriba.
                let az = (dx as f32).atan2(dz as f32);
                let elev = (p.y / d).asin();
                if az > -0.5 && az < 1.2 {
                    let _ = elev;
                    continue;
                }
                let mut b = block(c.x as i32 + dx, c.y as i32 + dy, c.z as i32 + dz, STONE);
                b.alt = Some((FAKE_SKY, c + Vec3::new(0.0, 1.0, 0.0)));
                o.push(b);
            }
        }
    }

}

fn build_towers(o: &mut Vec<Object>) {
    // Torre principal (detrás de la cúpula) con escalonamientos.
    // Se levanta en el centro de la cúpula y la atraviesa.
    let (tx, tz) = (DOME_C.x, DOME_C.z);
    o.push(cube_at(tx - 2.0, 1.0, tz - 1.5, 4.0, 3.0, 3.0, STONE));
    o.push(cube_at(tx - 1.5, 4.0, tz - 1.0, 3.0, 11.0, 2.0, STONE));
    o.push(cube_at(tx - 1.0, 15.0, tz - 0.75, 2.0, 3.0, 1.5, STONE));
    o.push(cube_at(tx - 0.4, 18.0, tz - 0.4, 0.8, 3.2, 0.8, STONE));
    // Franja de ventanas oscuras (obsidiana) en la torre.
    for k in 0..4 {
        o.push(cube_at(tx - 0.4, 6.0 + k as f32 * 2.2, tz + 1.0, 0.8, 1.2, 0.05, OBSIDIAN));
    }
    // Torres menores.
    o.push(cube_at(-10.5, 1.0, -10.5, 2.0, 9.0, 2.0, STONE));
    o.push(cube_at(-10.2, 10.0, -10.2, 1.4, 1.5, 1.4, STONE));
    o.push(cube_at(3.0, 1.0, -11.0, 2.0, 7.0, 2.0, STONE));
    o.push(cube_at(3.3, 8.0, -10.7, 1.4, 1.2, 1.4, STONE));

}

/// Zangetsu (forma Shikai): hoja de acero enorme clavada en una duna.
fn build_zangetsu(o: &mut Vec<Object>, base: Vec3) {
    let ground = sand_height(base.x.round() as i32, base.z.round() as i32) as f32;
    let pivot = Vec3::new(base.x, ground, base.z);
    // Casi de frente a la vista héroe y un poco echada hacia atrás: así la hoja refleja
    // el cielo que está detrás del espectador, donde se abre la Garganta roja.
    let rot = Mat3::rot_y(0.3).mul(&Mat3::rot_x(-0.24)).mul(&Mat3::rot_z(-0.12));
    // Hoja: ancha, delgada y larga; se hunde 1.5 bloques en la arena.
    let blade = Object::cube(
        Vec3::new(pivot.x - 0.75, ground - 1.5, pivot.z - 0.07),
        Vec3::new(pivot.x + 0.75, ground + 6.0, pivot.z + 0.07),
        STEEL,
    )
    .rotated(rot, pivot)
    .with_uv_scale(0.25);
    o.push(blade);
    // Lomo oscuro de la hoja (borde sin filo).
    o.push(
        Object::cube(
            Vec3::new(pivot.x - 0.9, ground - 1.5, pivot.z - 0.1),
            Vec3::new(pivot.x - 0.72, ground + 6.2, pivot.z + 0.1),
            OBSIDIAN,
        )
        .rotated(rot, pivot),
    );
    // Mango con vendaje.
    o.push(
        Object::cube(
            Vec3::new(pivot.x - 0.2, ground + 6.0, pivot.z - 0.18),
            Vec3::new(pivot.x + 0.2, ground + 8.2, pivot.z + 0.18),
            HILT,
        )
        .rotated(rot, pivot)
        .with_uv_scale(1.5),
    );
}

/// Árbol de cuarzo: tronco de prismas inclinados y ramas que se abren hacia arriba.
fn build_quartz_tree(o: &mut Vec<Object>, base: Vec3, seed: u32, scale: f32) {
    let rnd = |k: u32| (hash_u32(seed * 977 + k) & 0xFFFF) as f32 / 65535.0;
    let ground = sand_height(base.x.round() as i32, base.z.round() as i32) as f32;
    let mut p = Vec3::new(base.x, ground - 0.3, base.z);
    let seg_h = 1.7 * scale;
    let w = 0.55 * scale;
    let mut lean = Mat3::IDENTITY;
    // Tronco: 3 segmentos con ligera torsión.
    for s in 0..3 {
        let tilt = Mat3::rot_y(rnd(s * 3) * 2.0 * PI).mul(&Mat3::rot_z((rnd(s * 3 + 1) - 0.5) * 0.35));
        lean = lean.mul(&tilt);
        let seg = Object::cube(
            Vec3::new(p.x - w * 0.5, p.y, p.z - w * 0.5),
            Vec3::new(p.x + w * 0.5, p.y + seg_h, p.z + w * 0.5),
            QUARTZ,
        )
        .rotated(lean, p)
        .with_uv_scale(0.8);
        o.push(seg);
        p = p + lean.apply(Vec3::new(0.0, seg_h * 0.97, 0.0));
    }
    // Ramas.
    let n_branch = 4 + (seed % 2) as usize;
    for b in 0..n_branch {
        let yaw = (b as f32 / n_branch as f32) * 2.0 * PI + rnd(20 + b as u32) * 0.8;
        let pitch = 0.6 + rnd(30 + b as u32) * 0.5;
        let len = (1.6 + rnd(40 + b as u32) * 1.4) * scale;
        let bw = 0.32 * scale;
        let r = Mat3::rot_y(yaw).mul(&Mat3::rot_x(pitch));
        let start = p - Vec3::new(0.0, 0.8 * scale * rnd(50 + b as u32), 0.0);
        o.push(
            Object::cube(
                Vec3::new(start.x - bw * 0.5, start.y, start.z - bw * 0.5),
                Vec3::new(start.x + bw * 0.5, start.y + len, start.z + bw * 0.5),
                QUARTZ,
            )
            .rotated(r, start)
            .with_uv_scale(0.8),
        );
        // Punta de cristal girada 45° (brillo facetado).
        let tip = start + r.apply(Vec3::new(0.0, len, 0.0));
        let tw = 0.38 * scale;
        o.push(
            Object::cube(
                Vec3::new(tip.x - tw * 0.5, tip.y - tw * 0.5, tip.z - tw * 0.5),
                Vec3::new(tip.x + tw * 0.5, tip.y + tw * 0.5, tip.z + tw * 0.5),
                QUARTZ,
            )
            .rotated(Mat3::rot_y(0.785).mul(&Mat3::rot_x(0.615)), tip),
        );
    }
}

/// Racimo de 2-3 prismas de cuarzo clavados en la arena.
fn build_shard(o: &mut Vec<Object>, i: i32, j: i32, seed: u32) {
    let rnd = |k: u32| (hash_u32(seed * 131 + k + 7) & 0xFFFF) as f32 / 65535.0;
    let g = sand_height(i, j) as f32;
    for k in 0..(2 + seed % 2) {
        let base = Vec3::new(i as f32 + (rnd(k) - 0.5) * 0.8, g - 0.2, j as f32 + (rnd(k + 9) - 0.5) * 0.8);
        let h = 0.8 + rnd(k + 20) * 1.2;
        let w = 0.22 + rnd(k + 30) * 0.12;
        let r = Mat3::rot_y(rnd(k + 40) * PI).mul(&Mat3::rot_x((rnd(k + 50) - 0.5) * 0.9));
        o.push(
            Object::cube(Vec3::new(base.x - w, base.y, base.z - w), Vec3::new(base.x + w, base.y + h, base.z + w), QUARTZ)
                .rotated(r, base),
        );
    }
}

/// Máscara Hollow de hueso medio enterrada (acento rojo por la textura).
fn build_hollow_mask(o: &mut Vec<Object>, base: Vec3) {
    let ground = sand_height(base.x.round() as i32, base.z.round() as i32) as f32;
    let c = Vec3::new(base.x, ground + 0.3, base.z);
    let mut s = Object::sphere(c, 1.5, BONE);
    s.rot = Some((Mat3::rot_y(2.4).mul(&Mat3::rot_x(-0.35)), c));
    o.push(s);
    // Cuernos.
    for side in [-1.0f32, 1.0] {
        let hp = c + Mat3::rot_y(2.4).apply(Vec3::new(side * 0.9, 1.0, 0.2));
        o.push(
            Object::cube(Vec3::new(hp.x - 0.18, hp.y, hp.z - 0.18), Vec3::new(hp.x + 0.18, hp.y + 1.6, hp.z + 0.18), BONE)
                .rotated(Mat3::rot_y(2.4).mul(&Mat3::rot_z(-side * 0.5)), hp),
        );
    }
}

/// Ruinas: bloques y columnas caídas que dan escala y detalle intermedio.
fn build_ruins(o: &mut Vec<Object>) {
    let g = |i: i32, j: i32| sand_height(i, j) as f32;
    // Columna en pie.
    o.push(cube_at(7.5, g(8, -8) - 0.5, -8.5, 1.0, 4.5, 1.0, STONE));
    o.push(cube_at(7.3, g(8, -8) + 4.0, -8.7, 1.4, 0.5, 1.4, STONE));
    // Columna caída.
    let p = Vec3::new(1.0, g(1, -7) + 0.4, -6.5);
    o.push(
        Object::cube(Vec3::new(p.x - 3.0, p.y - 0.45, p.z - 0.45), Vec3::new(p.x + 3.0, p.y + 0.45, p.z + 0.45), STONE)
            .rotated(Mat3::rot_y(0.4).mul(&Mat3::rot_z(0.12)), p),
    );
    // Bloques sueltos.
    for (i, j, s) in [(6, 0, 0.9f32), (7, 1, 0.6), (-6, 4, 0.8), (11, -8, 1.0)] {
        let y = g(i, j) - 0.2;
        let c = Vec3::new(i as f32, y + s * 0.5, j as f32);
        o.push(
            Object::cube(c - Vec3::splat(s * 0.5), c + Vec3::splat(s * 0.5), STONE)
                .rotated(Mat3::rot_y(i as f32 * 0.7).mul(&Mat3::rot_x(0.2)), c),
        );
    }
}
