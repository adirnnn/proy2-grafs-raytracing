//! Construcción del diorama "Las Noches bajo la Luna Eterna".
//!
//! Referencia: Las Noches en Hueco Mundo (Bleach): un tambor blanco enorme con una
//! cúpula baja, seis torres cilíndricas con base acampanada, desierto blanco, árboles
//! muertos de cuarzo y una luna creciente bajo un cielo nublado.
//!
//! Unidades: 1 unidad = 1 bloque. Y hacia arriba. El diorama ocupa x,z en [-24.9, 24.9]
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
pub const MOON_DIR: Vec3 = Vec3::new(0.02, 0.21, -0.98);
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
            // Luz principal: el resplandor lunar difuso de las nubes, desde adelante-izquierda.
            Light::Directional {
                dir: Vec3::new(-0.50, 0.70, 0.52).normalized(),
                color: Vec3::new(0.86, 0.90, 1.0) * 1.15,
                spread: 0.045,
            },
            // Contraluz de la luna (detrás de Las Noches).
            Light::Directional { dir: MOON_DIR.normalized(), color: Vec3::new(0.80, 0.88, 1.0) * 1.1, spread: 0.02 },
            // Luz del "cielo falso" dentro de la cúpula: se escapa por el corte.
            Light::Point { pos: DOME_C + Vec3::new(0.0, 6.5, 0.0), color: Vec3::new(1.6, 2.1, 2.9), radius: 9.0 },
        ];
        Ok(Scene {
            objects,
            bvh,
            materials,
            textures,
            skybox,
            lights,
            ambient_sky: Vec3::new(0.075, 0.082, 0.092),
            ambient_ground: Vec3::new(0.060, 0.060, 0.062),
        })
    }
}

// ----------------------------------------------------------------------------
// Diseño del diorama
// ----------------------------------------------------------------------------

/// Eje vertical alrededor del cual gira el diorama (centro de Las Noches).
pub const PIVOT: Vec3 = Vec3::new(0.0, 0.0, -4.0);

pub const HALF: i32 = 24; // celdas de -24..=24 (49x49 bloques)
/// Centro de Las Noches (base del tambor).
pub const DOME_C: Vec3 = Vec3::new(0.0, 1.0, -4.0);
/// Radio exterior del tambor, altura del tambor y altura extra de la cúpula.
const DRUM_R: f32 = 10.5;
const DRUM_H: f32 = 8.0;
const DOME_H: f32 = 4.5;
/// Corte de maqueta (ángulo alrededor del centro, 0 = +z, 90° = +x): deja ver el interior.
const CUT_FROM: f32 = 180.0;
const CUT_TO: f32 = 255.0;

fn noise2(x: f32, z: f32, seed: u32) -> f32 {
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

fn dist_to_center(x: f32, z: f32) -> f32 {
    ((x - DOME_C.x).powi(2) + (z - DOME_C.z).powi(2)).sqrt()
}

/// Ángulo (grados, 0..360) de un punto alrededor del centro de Las Noches.
fn azimuth(x: f32, z: f32) -> f32 {
    (x - DOME_C.x).atan2(z - DOME_C.z).to_degrees().rem_euclid(360.0)
}

fn in_cut(x: f32, z: f32) -> bool {
    let a = azimuth(x, z);
    a > CUT_FROM && a < CUT_TO
}

/// Camino de arena aplanada desde la puerta hasta el borde frontal.
fn on_path(i: i32, j: i32) -> bool {
    i.abs() <= 2 && j as f32 > DOME_C.z + DRUM_R
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Altura de la arena (en bloques) en la celda (i, j).
pub fn sand_height(i: i32, j: i32) -> i32 {
    let (x, z) = (i as f32, j as f32);
    let n = noise2(x * 0.09, z * 0.09, 7) * 0.65 + noise2(x * 0.22, z * 0.22, 13) * 0.35;
    // Dunas que suben hacia los bordes y se aplanan cerca de la fortaleza y del camino.
    // (El frente queda bajo para que se vea el camino hacia la puerta.)
    let edge = smooth((x.abs().max(-z) - 12.0) / 12.0) + 0.3 * smooth((z - 14.0) / 10.0);
    let near = smooth((dist_to_center(x, z) - DRUM_R - 2.0) / 7.0);
    let path = if i.abs() <= 4 && z > DOME_C.z + DRUM_R {
        0.35 + 0.65 * smooth((i.abs() as f32 - 2.0) / 2.0)
    } else {
        1.0
    };
    (1.0 + (n * 3.2 + edge * 3.5) * near * path).floor() as i32
}

fn cube_at(x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, m: u16) -> Object {
    Object::cube(Vec3::new(x, y, z), Vec3::new(x + sx, y + sy, z + sz), m)
}

/// Columna de bloques de la celda (i, j) entre las alturas y0 y y1.
fn column(i: i32, j: i32, y0: f32, y1: f32, m: u16) -> Object {
    cube_at(i as f32 - 0.5, y0, j as f32 - 0.5, 1.0, y1 - y0, 1.0, m)
}

pub fn build_diorama() -> Vec<Object> {
    let mut o = Vec::new();

    // Zócalo de basalto con molduras de obsidiana.
    o.push(cube_at(-24.7, -4.0, -24.7, 49.4, 4.0, 49.4, BASALT));
    o.push(cube_at(-24.9, -0.8, -24.9, 49.8, 0.4, 49.8, OBSIDIAN).with_uv_scale(0.5));
    o.push(cube_at(-24.9, -3.9, -24.9, 49.8, 0.3, 49.8, OBSIDIAN).with_uv_scale(0.5));

    // Terreno: una columna por celda.
    for i in -HALF..=HALF {
        for j in -HALF..=HALF {
            let d = dist_to_center(i as f32, j as f32);
            if d < DRUM_R - 1.0 {
                // Piso interior: mármol pulido del salón.
                o.push(column(i, j, 0.0, 1.0, MARBLE));
            } else {
                let h = if on_path(i, j) { 1 } else { sand_height(i, j) };
                o.push(column(i, j, 0.0, h as f32, SAND));
            }
        }
    }

    build_las_noches(&mut o);

    // Seis torres con base acampanada: (x, z, radio, altura).
    for (x, z, r, h) in [
        (-11.5, 5.5, 2.6, 27.0),
        (-15.0, -4.5, 2.3, 23.0),
        (-16.5, 3.0, 2.2, 14.0),
        (12.0, 5.0, 2.8, 29.0),
        (9.0, -16.0, 2.3, 25.0),
        (15.0, -7.0, 1.9, 17.0),
    ] {
        build_tower(&mut o, x, z, r, h);
    }

    // Árboles muertos de cuarzo, escasos y delgados.
    for (k, (x, z, s)) in [
        (-14.0, 15.5, 1.7),
        (-21.0, 7.0, 1.0),
        (-8.0, 21.0, 0.8),
        (18.5, 15.0, 1.0),
        (21.0, -1.0, 0.85),
        (-21.0, -10.0, 0.95),
        (5.0, -21.5, 0.9),
        (12.5, 18.0, 1.35),
    ]
    .iter()
    .enumerate()
    {
        build_dead_tree(&mut o, Vec3::new(*x, 0.0, *z), k as u32 + 1, *s);
    }

    // Lápidas de concreto junto al camino.
    for (x, z, h) in [(-4.0, 14.0, 3.4f32), (4.5, 11.0, 3.0), (-5.0, 21.0, 2.2)] {
        let g = sand_height(x as i32, z as i32) as f32;
        o.push(cube_at(x - 0.4, g - 0.5, z - 0.3, 0.8, h + 0.5, 0.6, STONE));
    }
    // Monolitos de obsidiana pulida dispersos en el desierto: (x, z, tamaño, giro, inclinación).
    // Inclinados hacia atrás para que reflejen el cielo oscuro (y la Garganta) y no la arena.
    for (x, z, sx, sy, sz, a, tilt) in [
        (17.0, 9.0, 2.2f32, 1.8f32, 1.6f32, 0.3f32, 0.0f32),
        (20.5, 4.0, 1.5, 1.5, 1.5, 0.8, 0.0),
        (-18.0, -17.0, 2.4, 2.0, 1.6, -0.4, 0.0),
        (-8.0, 10.5, 2.6, 3.0, 0.7, -0.5, -0.38),
    ] {
        let g = sand_height(x as i32, z as i32) as f32;
        let c = Vec3::new(x, g - 0.3 + sy * 0.5, z);
        o.push(
            Object::cube(c - Vec3::new(sx, sy, sz) * 0.5, c + Vec3::new(sx, sy, sz) * 0.5, OBSIDIAN)
                .rotated(Mat3::rot_y(a).mul(&Mat3::rot_x(tilt)), c),
        );
    }
    o
}

/// Las Noches: tambor de bloques, cornisa, cúpula baja escalonada, cinco torrecillas,
/// puerta frontal, y un corte de maqueta que revela el cielo falso y el salón del trono.
fn build_las_noches(o: &mut Vec<Object>) {
    let c = DOME_C;
    let inner = c + Vec3::new(0.0, 2.0, 0.0);
    // Medio bloque por celda: las curvas del tambor y la cúpula se ven más redondas.
    const CS: f32 = 0.5;
    let r = ((DRUM_R + 1.0) / CS).ceil() as i32;
    // Las caras que miran hacia el interior usan el material "cielo falso".
    let shell = |mut b: Object| {
        b.alt = Some((FAKE_SKY, inner));
        b
    };
    let top = |d: f32| {
        c.y + DRUM_H + 0.7 + (DOME_H * (1.0 - (d / DRUM_R).powi(2)).max(0.0).sqrt() / CS).round() * CS
    };
    let cell = |x: f32, z: f32, y0: f32, y1: f32| cube_at(x - CS * 0.5, y0, z - CS * 0.5, CS, y1 - y0, CS, STONE);
    for di in -r..=r {
        for dj in -r..=r {
            let (x, z) = (c.x + di as f32 * CS, c.z + dj as f32 * CS);
            let d = dist_to_center(x, z);
            if in_cut(x, z) {
                continue;
            }
            // Pared del tambor.
            if d >= DRUM_R - 1.2 && d < DRUM_R {
                o.push(shell(cell(x, z, 1.0, c.y + DRUM_H)));
            }
            // Cornisa que sobresale.
            if d >= DRUM_R - 1.2 && d < DRUM_R + 0.7 {
                o.push(shell(cell(x, z, c.y + DRUM_H, c.y + DRUM_H + 0.7)));
            }
            // Cúpula escalonada: columnas macizas desde la parte alta del tambor.
            // (Su cara inferior forma el techo plano del "cielo falso".)
            if d < DRUM_R - 0.2 {
                o.push(shell(cell(x, z, c.y + DRUM_H, top(d))));
            }
        }
    }
    let roof = c.y + DRUM_H + 0.7 + DOME_H;
    // Cinco torrecillas sobre la cúpula.
    for k in -2..=2 {
        let x = c.x + k as f32 * 1.1;
        let h = 2.6 - (k as f32).abs() * 0.4;
        o.push(cube_at(x - 0.35, roof - 0.5, c.z - 0.5, 0.7, h, 1.0, STONE));
    }
    // Puerta frontal (bloque con vano oscuro).
    let front = c.z + DRUM_R;
    o.push(cube_at(c.x - 2.8, 0.0, front - 1.5, 5.6, 5.5, 3.5, STONE));
    o.push(cube_at(c.x - 3.1, 5.5, front - 1.5, 6.2, 0.5, 3.8, STONE));
    o.push(cube_at(c.x - 0.9, 1.0, front + 1.95, 1.8, 3.2, 0.08, OBSIDIAN));

    // Interior: estrado y trono.
    o.push(cube_at(c.x - 3.0, 1.0, c.z - 5.0, 6.0, 0.5, 4.0, STONE));
    o.push(cube_at(c.x - 2.0, 1.5, c.z - 4.6, 4.0, 0.5, 3.2, STONE));
    o.push(cube_at(c.x - 0.8, 2.0, c.z - 3.6, 1.6, 0.8, 1.4, STONE));
    o.push(cube_at(c.x - 0.8, 2.0, c.z - 4.2, 1.6, 4.5, 0.6, STONE));
    // Columnas interiores.
    for a in [0.0f32, 60.0, 120.0, 180.0, 240.0, 300.0] {
        let (s, co) = a.to_radians().sin_cos();
        let (x, z) = (c.x + s * 6.5, c.z + co * 6.5);
        o.push(cube_at(x - 0.5, 1.0, z - 0.5, 1.0, DRUM_H, 1.0, STONE));
    }
}

/// Torre cilíndrica de bloques con base acampanada. Los niveles consecutivos de una
/// misma celda se unen en un solo bloque alto para mantener pocos objetos.
fn build_tower(o: &mut Vec<Object>, cx: f32, cz: f32, r0: f32, h: f32) {
    const CS: f32 = 0.5; // medio bloque por celda y por nivel
    let radius = |y: f32| r0 + 3.0 * (-y / 3.5).exp();
    let base = sand_height(cx as i32, cz as i32) as f32 - 0.5;
    let rmax = (radius(0.0) / CS).ceil() as i32 + 1;
    let levels = (h / CS) as i32;
    for di in -rmax..=rmax {
        for dj in -rmax..=rmax {
            let (x, z) = (cx + di as f32 * CS, cz + dj as f32 * CS);
            let d = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
            let mut run: Option<i32> = None;
            for l in 0..=levels {
                let y = (l as f32 + 0.5) * CS;
                let inside = if l == levels {
                    false
                } else if l >= levels - 2 {
                    d < radius(y) // tapa superior
                } else {
                    let r = radius(y);
                    d < r && d >= r - 1.0
                };
                match (inside, run) {
                    (true, None) => run = Some(l),
                    (false, Some(s)) => {
                        let (y0, y1) = (base + s as f32 * CS, base + l as f32 * CS);
                        o.push(cube_at(x - CS * 0.5, y0, z - CS * 0.5, CS, y1 - y0, CS, STONE));
                        run = None;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Árbol muerto de cuarzo: tronco delgado y torcido, ramas que se dividen dos veces.
fn build_dead_tree(o: &mut Vec<Object>, base: Vec3, seed: u32, scale: f32) {
    let rnd = |k: u32| (hash_u32(seed * 977 + k) & 0xFFFF) as f32 / 65535.0;
    let ground = sand_height(base.x.round() as i32, base.z.round() as i32) as f32;
    let mut p = Vec3::new(base.x, ground - 0.4, base.z);
    let mut rot = Mat3::IDENTITY;
    let mut w = 0.85 * scale;
    for s in 0..4 {
        rot = rot.mul(&Mat3::rot_y(rnd(s) * 2.0 * PI).mul(&Mat3::rot_z((rnd(s + 10) - 0.5) * 0.35)));
        let len = (2.0 + rnd(s + 20)) * scale;
        push_branch(o, p, rot, w, len);
        p = p + rot.apply(Vec3::new(0.0, len * 0.95, 0.0));
        w *= 0.82;
        // Ramas laterales en los dos segmentos superiores.
        if s >= 2 {
            for b in 0..2u32 {
                let k = s * 10 + b;
                let br = Mat3::rot_y(rnd(30 + k) * 2.0 * PI).mul(&Mat3::rot_x(0.5 + rnd(40 + k) * 0.5));
                let l1 = (1.6 + rnd(50 + k) * 1.4) * scale;
                push_branch(o, p, br, w * 0.6, l1);
                let tip = p + br.apply(Vec3::new(0.0, l1 * 0.95, 0.0));
                let br2 = br.mul(&Mat3::rot_x(-0.35 - rnd(60 + k) * 0.3));
                push_branch(o, tip, br2, w * 0.4, l1 * 0.6);
            }
        }
    }
    // Copa: dos ramitas finales.
    for b in 0..2u32 {
        let br = rot.mul(&Mat3::rot_y(b as f32 * PI + rnd(70)).mul(&Mat3::rot_x(0.4)));
        push_branch(o, p, br, w * 0.5, 1.4 * scale);
    }
}

fn push_branch(o: &mut Vec<Object>, start: Vec3, rot: Mat3, w: f32, len: f32) {
    o.push(
        Object::cube(
            Vec3::new(start.x - w * 0.5, start.y, start.z - w * 0.5),
            Vec3::new(start.x + w * 0.5, start.y + len, start.z + w * 0.5),
            QUARTZ,
        )
        .rotated(rot, start)
        .with_uv_scale(0.6),
    );
}
