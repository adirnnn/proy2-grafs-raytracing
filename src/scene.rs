//! Construcción del diorama "Las Noches bajo la Luna Eterna".
//!
//! Referencia: Las Noches en Hueco Mundo (Bleach): un tambor blanco enorme con una
//! cúpula baja, seis torres cilíndricas con base acampanada, desierto blanco, árboles
//! muertos de cuarzo y una luna creciente bajo un cielo nublado.
//!
//! Escala: 1 unidad ≈ 4 m. Y hacia arriba. El suelo del desierto está en y = 1 y se
//! extiende hasta perderse en la niebla (no hay bordes visibles). La fortaleza mide
//! ~85 m de diámetro y sus torres llegan a ~115 m; Zangetsu mide ~2 m (0.53 u).

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
    /// Luz puntual con caída suave a partir de `radius` (alcance máximo 6·radius).
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
    /// Densidad de la niebla por unidad de distancia (0 = sin niebla).
    pub fog_density: f32,
}

/// Dirección (en el mundo) hacia la luna dibujada en el skybox.
/// El generador de texturas usa la misma constante para pintar la luna ahí.
pub const MOON_DIR: Vec3 = Vec3::new(0.02, 0.36, -0.93);
/// Dirección de la Garganta (grieta roja) en el cielo: detrás de la cámara.
pub const GARGANTA_DIR: Vec3 = Vec3::new(0.18, 0.24, 0.95);
/// Eje vertical alrededor del cual gira el diorama (centro de Las Noches).
pub const PIVOT: Vec3 = Vec3::new(0.0, 0.0, -4.0);
/// Altura del suelo plano del desierto.
pub const GROUND_Y: f32 = 1.0;

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
        let gate = Vec3::new(DOME_C.x, 3.2, DOME_C.z + DRUM_R + 3.2);
        let lights = vec![
            // Resplandor lunar difuso de las nubes: luz principal tenue, sombras suaves.
            Light::Directional {
                dir: Vec3::new(-0.45, 0.62, 0.64).normalized(),
                color: Vec3::new(0.70, 0.76, 0.88) * 0.38,
                spread: 0.05,
            },
            // Contraluz de la luna (detrás de Las Noches): recorta las siluetas.
            Light::Directional { dir: MOON_DIR.normalized(), color: Vec3::new(0.80, 0.88, 1.0) * 0.9, spread: 0.02 },
            // El "cielo falso" de adentro: se derrama por el corte de la cúpula.
            Light::Point { pos: DOME_C + Vec3::new(-2.5, 6.0, -2.5), color: Vec3::new(1.7, 2.3, 3.2) * 1.4, radius: 7.0 },
            // La puerta abierta: luz del interior que cae sobre la calzada.
            Light::Point { pos: gate, color: Vec3::new(1.5, 2.0, 2.7), radius: 3.5 },
        ];
        Ok(Scene {
            objects,
            bvh,
            materials,
            textures,
            skybox,
            lights,
            ambient_sky: Vec3::new(0.030, 0.034, 0.040),
            ambient_ground: Vec3::new(0.026, 0.026, 0.028),
            fog_density: 0.0065,
        })
    }
}

// ----------------------------------------------------------------------------
// Diseño del diorama
// ----------------------------------------------------------------------------

/// Celdas de terreno con dunas: de -HALF a HALF (el resto es el suelo plano infinito).
pub const HALF: i32 = 44;
/// Centro de Las Noches (base del tambor).
pub const DOME_C: Vec3 = Vec3::new(0.0, 1.0, -4.0);
/// Radio exterior del tambor, altura del tambor y altura extra de la cúpula.
const DRUM_R: f32 = 10.5;
const DRUM_H: f32 = 8.0;
const DOME_H: f32 = 4.5;
/// Tamaño de celda de la arquitectura (cuarto de bloque: curvas suaves).
const CS: f32 = 0.25;
/// Corte de maqueta (ángulo alrededor del centro, 0 = +z, 90° = +x): deja ver el interior.
const CUT_FROM: f32 = 180.0;
const CUT_TO: f32 = 255.0;
/// Calzada de obsidiana: de la puerta hacia el frente, con este medio ancho.
const CAUSEWAY_HALF_W: f32 = 3.0;

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

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn on_causeway(x: f32, z: f32) -> bool {
    x.abs() <= CAUSEWAY_HALF_W && z > DOME_C.z + DRUM_R
}

/// Altura de la arena (múltiplos de medio bloque) en la celda (i, j).
pub fn sand_height(i: i32, j: i32) -> f32 {
    let (x, z) = (i as f32, j as f32);
    let n = noise2(x * 0.07, z * 0.07, 7) * 0.65 + noise2(x * 0.19, z * 0.19, 13) * 0.35;
    let r = dist_to_center(x, z);
    // Dunas en un anillo alrededor de la fortaleza; planas cerca de ella, cerca de la
    // calzada y lejos (donde se funden con el suelo infinito bajo la niebla).
    let near = smooth((r - DRUM_R - 2.0) / 8.0);
    let far = 1.0 - smooth((r - 26.0) / 14.0);
    let path = 1.0
        - (1.0 - smooth((x.abs() - CAUSEWAY_HALF_W - 0.5) / 5.0)) * smooth((z - DOME_C.z - DRUM_R + 2.0) / 3.0);
    let h = n * 4.5 * near * far * path;
    GROUND_Y + (h * 2.0).floor() * 0.5
}

fn cube_at(x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, m: u16) -> Object {
    Object::cube(Vec3::new(x, y, z), Vec3::new(x + sx, y + sy, z + sz), m)
}

pub fn build_diorama() -> Vec<Object> {
    let mut o = Vec::new();

    // Suelo del desierto: un cubo enorme (8 km de lado) que se pierde en la niebla.
    o.push(cube_at(-1000.0, -6.0, -1000.0, 2000.0, 6.0 + GROUND_Y, 2000.0, SAND));

    // Dunas: una columna por celda donde la arena sube sobre el suelo.
    for i in -HALF..=HALF {
        for j in -HALF..=HALF {
            let (x, z) = (i as f32, j as f32);
            if dist_to_center(x, z) < DRUM_R + 0.5 || on_causeway(x, z) {
                continue;
            }
            let h = sand_height(i, j);
            if h > GROUND_Y {
                o.push(cube_at(x - 0.5, GROUND_Y - 0.5, z - 0.5, 1.0, h - GROUND_Y + 0.5, 1.0, SAND));
            }
        }
    }

    build_las_noches(&mut o);
    build_causeway(&mut o);

    // Seis torres con base acampanada: (x, z, radio, altura).
    for (k, (x, z, r, h)) in [
        (-11.5, 5.5, 2.6, 27.0),
        (-15.0, -4.5, 2.3, 23.0),
        (-16.5, 3.0, 2.2, 14.0),
        (12.0, 5.0, 2.8, 29.0),
        (9.0, -16.0, 2.3, 25.0),
        (15.0, -7.0, 1.9, 17.0),
    ]
    .iter()
    .enumerate()
    {
        build_tower(&mut o, *x, *z, *r, *h, k as u32);
    }

    // Árboles muertos de cuarzo, dispersos (algunos lejos, entre la niebla).
    for (k, (x, z, s)) in [
        (-14.0, 15.5, 1.7),
        (-21.0, 7.0, 1.0),
        (-8.0, 21.0, 0.8),
        (18.5, 15.0, 1.0),
        (21.0, -1.0, 0.85),
        (-21.0, -10.0, 0.95),
        (5.0, -21.5, 0.9),
        (12.5, 18.0, 1.35),
        (-30.0, 24.0, 1.2),
        (33.0, 20.0, 1.4),
        (-36.0, -8.0, 1.1),
        (28.0, -26.0, 1.3),
        (-9.0, 36.0, 0.9),
    ]
    .iter()
    .enumerate()
    {
        build_dead_tree(&mut o, Vec3::new(*x, 0.0, *z), k as u32 + 1, *s);
    }

    // Lápidas de concreto junto a la calzada.
    for (x, z, h) in [(-5.0, 14.0, 3.4f32), (5.5, 11.0, 3.0), (-5.5, 22.0, 2.2), (6.0, 27.0, 2.6), (-5.5, 33.0, 1.8)] {
        let g = sand_height(x as i32, z as i32);
        o.push(cube_at(x - 0.4, g - 0.5, z - 0.3, 0.8, h + 0.5, 0.6, STONE));
    }
    // Monolitos de obsidiana pulida. Algunos inclinados hacia atrás para reflejar el cielo.
    for (x, z, sx, sy, sz, a, tilt) in [
        (17.0, 9.0, 2.2f32, 1.8f32, 1.6f32, 0.3f32, 0.0f32),
        (20.5, 4.0, 1.5, 1.5, 1.5, 0.8, 0.0),
        (-18.0, -17.0, 2.4, 2.0, 1.6, -0.4, 0.0),
        (-9.5, 12.0, 2.6, 3.0, 0.7, -0.5, -0.38),
    ] {
        let g = sand_height(x as i32, z as i32);
        let c = Vec3::new(x, g - 0.3 + sy * 0.5, z);
        o.push(
            Object::cube(c - Vec3::new(sx, sy, sz) * 0.5, c + Vec3::new(sx, sy, sz) * 0.5, OBSIDIAN)
                .rotated(Mat3::rot_y(a).mul(&Mat3::rot_x(tilt)), c),
        );
    }

    // Zangetsu, a escala humana, clavada en la arena junto a la calzada (primer plano).
    build_zangetsu(&mut o, Vec3::new(1.6, GROUND_Y, 42.2));
    o
}

/// Calzada de losas de obsidiana pulida con bordillo de concreto, de la puerta al frente.
fn build_causeway(o: &mut Vec<Object>) {
    let z0 = DOME_C.z + DRUM_R + 1.8;
    let z1 = 60.0;
    let w = CAUSEWAY_HALF_W;
    let mut z = z0;
    while z < z1 {
        // Losas de 2 bloques de largo, unidas (la textura marca las juntas).
        o.push(cube_at(-w + 0.3, GROUND_Y - 0.5, z, 2.0 * w - 0.6, 0.56, 2.0, OBSIDIAN).with_uv_scale(0.5));
        z += 2.0;
    }
    // Bordillos.
    for side in [-1.0f32, 1.0] {
        o.push(cube_at(side * w - 0.3, GROUND_Y - 0.5, z0, 0.6, 0.75, z1 - z0, STONE));
    }
}

/// Las Noches: tambor, cornisa, cúpula baja escalonada, cinco torrecillas, puerta
/// iluminada, y un corte de maqueta que revela el cielo falso y el salón del trono.
fn build_las_noches(o: &mut Vec<Object>) {
    let c = DOME_C;
    let inner = c + Vec3::new(0.0, 2.0, 0.0);
    let r = ((DRUM_R + 1.0) / CS).ceil() as i32;
    // Las caras que miran hacia el interior usan el material "cielo falso".
    let shell = |mut b: Object| {
        b.alt = Some((FAKE_SKY, inner));
        b
    };
    let top = |d: f32| {
        c.y + DRUM_H + 0.7 + (DOME_H * (1.0 - (d / DRUM_R).powi(2)).max(0.0).sqrt() / CS).round() * CS
    };
    let cell = |x: f32, z: f32, y0: f32, y1: f32, m: u16| cube_at(x - CS * 0.5, y0, z - CS * 0.5, CS, y1 - y0, CS, m);
    for di in -r..=r {
        for dj in -r..=r {
            let (x, z) = (c.x + di as f32 * CS, c.z + dj as f32 * CS);
            let d = dist_to_center(x, z);
            if d < DRUM_R - 1.0 {
                // Piso interior de mármol pulido (también bajo el corte).
                o.push(cell(x, z, 0.0, c.y, MARBLE));
            }
            if in_cut(x, z) {
                continue;
            }
            // Pared del tambor, con una banda de ventanas encendidas en la cara exterior.
            if d >= DRUM_R - 1.2 && d < DRUM_R {
                let window = d >= DRUM_R - CS && (azimuth(x, z) / 7.5).fract() < 0.3;
                if window {
                    o.push(shell(cell(x, z, 0.0, 5.6, STONE)));
                    o.push(cell(x, z, 5.6, 6.3, LIGHT));
                    o.push(shell(cell(x, z, 6.3, c.y + DRUM_H, STONE)));
                } else {
                    o.push(shell(cell(x, z, 0.0, c.y + DRUM_H, STONE)));
                }
            }
            // Cornisa que sobresale.
            if d >= DRUM_R - 1.2 && d < DRUM_R + 0.7 {
                o.push(shell(cell(x, z, c.y + DRUM_H, c.y + DRUM_H + 0.7, STONE)));
            }
            // Cúpula escalonada: columnas macizas desde la parte alta del tambor
            // (su cara inferior forma el techo del "cielo falso").
            if d < DRUM_R - 0.2 {
                o.push(shell(cell(x, z, c.y + DRUM_H, top(d), STONE)));
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
    // Puerta frontal con el vano iluminado desde adentro.
    let front = c.z + DRUM_R;
    o.push(cube_at(c.x - 2.8, 0.0, front - 1.5, 1.9, 5.5, 3.5, STONE));
    o.push(cube_at(c.x + 0.9, 0.0, front - 1.5, 1.9, 5.5, 3.5, STONE));
    o.push(cube_at(c.x - 0.9, 4.2, front - 1.5, 1.8, 1.3, 3.5, STONE));
    o.push(cube_at(c.x - 3.1, 5.5, front - 1.5, 6.2, 0.5, 3.8, STONE));
    o.push(cube_at(c.x - 0.9, 1.0, front - 1.4, 1.8, 3.2, 0.1, LIGHT));

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

/// Torre cilíndrica de bloques con base acampanada y ventanas rasgadas encendidas.
/// Los niveles consecutivos iguales de una misma celda se unen en un solo bloque alto.
fn build_tower(o: &mut Vec<Object>, cx: f32, cz: f32, r0: f32, h: f32, seed: u32) {
    const TS: f32 = 0.25; // cuarto de bloque por celda y por nivel
    let radius = |y: f32| r0 + 3.0 * (-y / 3.5).exp();
    let base = GROUND_Y - 0.5;
    let rmax = (radius(0.0) / TS).ceil() as i32 + 1;
    let levels = (h / TS) as i32;
    // Ventanas: rendijas verticales a ciertas alturas y en un ángulo propio de cada torre.
    let win_angle = (hash_u32(seed * 31 + 5) % 360) as f32;
    let is_window = |x: f32, z: f32, y: f32, d: f32, r: f32| {
        if d < r - TS || y < 8.0 {
            return false;
        }
        let a = (x - cx).atan2(z - cz).to_degrees().rem_euclid(360.0);
        let da = ((a - win_angle + 540.0) % 360.0 - 180.0).abs();
        let band = ((y - 8.0) / 5.0).fract();
        da < 7.0 && band > 0.15 && band < 0.55
    };
    for di in -rmax..=rmax {
        for dj in -rmax..=rmax {
            let (x, z) = (cx + di as f32 * TS, cz + dj as f32 * TS);
            let d = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
            // Cada nivel es: 0 = vacío, 1 = muro, 2 = ventana.
            let mut run: Option<(i32, u8)> = None;
            for l in 0..=levels {
                let y = (l as f32 + 0.5) * TS;
                let kind: u8 = if l == levels {
                    0
                } else if l >= levels - 2 {
                    (d < radius(y)) as u8
                } else {
                    let r = radius(y);
                    if d < r && d >= r - 0.75 {
                        if is_window(x, z, y, d, r) { 2 } else { 1 }
                    } else {
                        0
                    }
                };
                match run {
                    Some((_, k)) if k == kind => {}
                    Some((s, k)) => {
                        let (y0, y1) = (base + s as f32 * TS, base + l as f32 * TS);
                        let m = if k == 2 { LIGHT } else { STONE };
                        o.push(cube_at(x - TS * 0.5, y0, z - TS * 0.5, TS, y1 - y0, TS, m));
                        run = if kind > 0 { Some((l, kind)) } else { None };
                    }
                    None if kind > 0 => run = Some((l, kind)),
                    None => {}
                }
            }
        }
    }
}

/// Árbol muerto de cuarzo: tronco torcido y ramas que se dividen dos veces.
fn build_dead_tree(o: &mut Vec<Object>, base: Vec3, seed: u32, scale: f32) {
    let rnd = |k: u32| (hash_u32(seed * 977 + k) & 0xFFFF) as f32 / 65535.0;
    let ground = sand_height(base.x.round() as i32, base.z.round() as i32);
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

/// Zangetsu (Shikai) a escala real (1 u ≈ 4 m): hoja de ~1.6 m x 0.4 m, sin guarda,
/// con el mango vendado. Clavada e inclinada en la arena.
pub fn build_zangetsu(o: &mut Vec<Object>, p: Vec3) {
    let rot = Mat3::rot_y(0.35).mul(&Mat3::rot_x(-0.2)).mul(&Mat3::rot_z(0.12));
    let blade_w = 0.10;
    let blade_l = 0.40;
    let t = 0.012;
    let sunk = 0.08;
    // Hoja de acero.
    o.push(
        Object::cube(
            Vec3::new(p.x - blade_w * 0.5, p.y - sunk, p.z - t * 0.5),
            Vec3::new(p.x + blade_w * 0.5, p.y + blade_l - sunk, p.z + t * 0.5),
            STEEL,
        )
        .rotated(rot, p)
        .with_uv_scale(4.0),
    );
    // Lomo oscuro.
    o.push(
        Object::cube(
            Vec3::new(p.x - blade_w * 0.5 - 0.012, p.y - sunk, p.z - t * 0.7),
            Vec3::new(p.x - blade_w * 0.5, p.y + blade_l - sunk + 0.01, p.z + t * 0.7),
            OBSIDIAN,
        )
        .rotated(rot, p),
    );
    // Mango vendado.
    o.push(
        Object::cube(
            Vec3::new(p.x - 0.014, p.y + blade_l - sunk, p.z - 0.014),
            Vec3::new(p.x + 0.014, p.y + blade_l - sunk + 0.13, p.z + 0.014),
            HILT,
        )
        .rotated(rot, p)
        .with_uv_scale(20.0),
    );
}
