//! Generador procedural de las texturas y del skybox (archivos BMP en assets/).
//! Todo el arte del proyecto es original y sale de este programa:
//!     cargo run --release --bin gen_textures

use las_noches::image_io::{save_bmp, Rgb8};
use las_noches::math::{hash_u32, smoothstep, Vec3};
use las_noches::scene::{GARGANTA_DIR, MOON_DIR};
use las_noches::texture::{face_dir, SKY_FACES};
use std::path::Path;

const TEX: usize = 128;
const SKY: usize = 512;

fn h01(x: i32, y: i32, seed: u32) -> f32 {
    (hash_u32((x as u32).wrapping_mul(73_856_093) ^ (y as u32).wrapping_mul(19_349_663) ^ seed.wrapping_mul(83_492_791))
        & 0xFFFF) as f32
        / 65535.0
}

/// Ruido de valor periódico (período `p` celdas) => texturas sin costuras al repetirse.
fn vnoise(x: f32, y: f32, p: i32, seed: u32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - xi as f32, y - yi as f32);
    let w = |a: i32, b: i32| h01(a.rem_euclid(p), b.rem_euclid(p), seed);
    let (sx, sy) = (smoothstep(0.0, 1.0, fx), smoothstep(0.0, 1.0, fy));
    let a = w(xi, yi) + (w(xi + 1, yi) - w(xi, yi)) * sx;
    let b = w(xi, yi + 1) + (w(xi + 1, yi + 1) - w(xi, yi + 1)) * sx;
    a + (b - a) * sy
}

/// Ruido fractal periódico sobre coordenadas u,v en [0,1).
fn fbm(u: f32, v: f32, base: i32, oct: u32, seed: u32) -> f32 {
    let (mut sum, mut amp, mut norm, mut p) = (0.0, 0.5, 0.0, base);
    for o in 0..oct {
        sum += amp * vnoise(u * p as f32, v * p as f32, p, seed + o * 31);
        norm += amp;
        amp *= 0.5;
        p *= 2;
    }
    sum / norm
}

fn to8(c: Vec3) -> [u8; 3] {
    // Los valores se escriben directamente como sRGB (lo que "se ve" en un editor).
    let f = |x: f32| (x.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    [f(c.x), f(c.y), f(c.z)]
}

fn make(name: &str, f: impl Fn(f32, f32, usize, usize) -> Vec3) {
    let mut img = Rgb8::new(TEX, TEX);
    for y in 0..TEX {
        for x in 0..TEX {
            let u = (x as f32 + 0.5) / TEX as f32;
            let v = (y as f32 + 0.5) / TEX as f32;
            img.put(x, y, to8(f(u, v, x, y)));
        }
    }
    let path = Path::new("assets/textures").join(format!("{name}.bmp"));
    save_bmp(&path, &img).expect("no se pudo escribir la textura");
    println!("  {}", path.display());
}

/// Coordenada "pixel-art": 16x16 píxeles lógicos por bloque (look de bloques).
fn px16(x: usize, y: usize) -> (i32, i32) {
    ((x * 16 / TEX) as i32, (y * 16 / TEX) as i32)
}

fn gen_block_textures() {
    // 1. Arena: píxeles lógicos en tonos hueso con granos y ondas suaves.
    make("sand", |u, v, x, y| {
        let (px, py) = px16(x, y);
        let n = h01(px, py, 1);
        let ripple = ((v * 3.0 + fbm(u, v, 2, 2, 5) * 0.6) * std::f32::consts::TAU).sin() * 0.5 + 0.5;
        let base = Vec3::new(0.84, 0.76, 0.62) * (0.90 + 0.10 * n - 0.06 * ripple);
        if n > 0.93 { Vec3::new(0.66, 0.64, 0.66) } else { base }
    });
    // 2. Piedra de Las Noches: bloques de yeso blanco con juntas y vetas verticales.
    make("stone", |u, v, x, y| {
        let (px, py) = px16(x, y);
        let seam = x < 3 || y < 3 || (y > TEX / 2 - 2 && y < TEX / 2 + 1 && px < 16);
        let offset = if py >= 8 { 8 } else { 0 };
        let brick_seam = (px + offset) % 16 == 0 && x % 8 < 2;
        let n = fbm(u, v, 4, 3, 2);
        let streak = fbm(u * 0.5, v * 0.05, 8, 2, 9);
        let c = Vec3::new(0.90, 0.89, 0.86) * (0.86 + 0.12 * n + 0.05 * streak);
        if seam || brick_seam { c * 0.72 } else { c }
    });
    // 3. Cuarzo: blanco-cian con facetas diagonales y bordes brillantes.
    make("quartz", |u, v, _, _| {
        let facet = ((u + v * 0.6) * 5.0 + fbm(u, v, 2, 2, 3) * 1.5).fract();
        let band = smoothstep(0.0, 0.08, facet) * (1.0 - smoothstep(0.85, 1.0, facet));
        let edge = 1.0 - smoothstep(0.0, 0.06, u.min(1.0 - u).min(v).min(1.0 - v));
        let c = Vec3::new(0.80, 0.94, 1.0) * (0.85 + 0.15 * band);
        c.lerp(Vec3::ONE, edge * 0.8)
    });
    // 4. Obsidiana: negro violáceo con vetas brillantes.
    make("obsidian", |u, v, _, _| {
        let n = fbm(u, v, 3, 4, 4);
        let vein = 1.0 - smoothstep(0.0, 0.035, (n - 0.5).abs());
        let base = Vec3::new(0.05, 0.04, 0.08) * (0.8 + 0.4 * fbm(u, v, 8, 2, 8));
        base.lerp(Vec3::new(0.42, 0.30, 0.62), vein * 0.8)
    });
    // 5. Acero cepillado: vetas horizontales finas, un poco azulado.
    make("steel", |u, v, _, _| {
        let brush = fbm(u * 0.25, v * 8.0, 4, 3, 6);
        let fine = h01((u * 4.0 * TEX as f32) as i32 / 8, (v * TEX as f32) as i32, 12);
        Vec3::new(0.72, 0.75, 0.80) * (0.82 + 0.14 * brush + 0.05 * fine)
    });
    // 6. Cielo falso: azul diurno con nubes (interior de la cúpula).
    make("fake_sky", |u, v, _, _| {
        let cloud = smoothstep(0.52, 0.72, fbm(u, v, 2, 5, 21));
        let sky = Vec3::new(0.38, 0.62, 0.95).lerp(Vec3::new(0.62, 0.80, 1.0), v);
        sky.lerp(Vec3::new(0.97, 0.98, 1.0), cloud)
    });
    // 7. Hueso: blanco cálido, grietas y marcas rojas de máscara Hollow.
    make("bone", |u, v, _, _| {
        let n = fbm(u, v, 4, 4, 30);
        let crack = 1.0 - smoothstep(0.0, 0.02, (fbm(u, v, 3, 3, 33) - 0.5).abs());
        let mut c = Vec3::new(0.93, 0.90, 0.84) * (0.9 + 0.1 * n);
        c = c.lerp(Vec3::new(0.45, 0.40, 0.36), crack * 0.7);
        // Tres franjas rojas verticales en el "frente" de la máscara.
        for s in [0.40f32, 0.5, 0.60] {
            let d = (u - s).abs();
            if d < 0.018 && v > 0.35 && v < 0.85 {
                c = Vec3::new(0.72, 0.05, 0.07);
            }
        }
        c
    });
    // 8. Basalto: gris azulado oscuro, rugoso y pixelado.
    make("basalt", |u, v, x, y| {
        let (px, py) = px16(x, y);
        let n = h01(px, py, 40) * 0.5 + fbm(u, v, 4, 3, 41) * 0.5;
        let seam = x < 2 || y < 2;
        let c = Vec3::new(0.20, 0.21, 0.26) * (0.75 + 0.5 * n);
        if seam { c * 0.6 } else { c }
    });
    // 9. Vendaje del mango: tela blanca en diagonal con separaciones oscuras.
    make("hilt", |u, v, _, _| {
        let d = ((u + v) * 4.0).fract();
        let gap = smoothstep(0.0, 0.08, d) * (1.0 - smoothstep(0.9, 1.0, d));
        Vec3::new(0.88, 0.86, 0.82).lerp(Vec3::new(0.12, 0.10, 0.10), 1.0 - gap)
    });
}

/// Cielo nocturno de Hueco Mundo: degradado índigo, estrellas, luna creciente enorme
/// y una grieta roja (Garganta) como acento. `d` normalizada, en el mundo.
fn sky_color(d: Vec3) -> Vec3 {
    let up = d.y;
    let zenith = Vec3::new(0.008, 0.010, 0.030);
    let horizon = Vec3::new(0.022, 0.020, 0.050);
    let mut c = if up >= 0.0 {
        horizon.lerp(zenith, smoothstep(0.0, 0.6, up))
    } else {
        // Bajo el horizonte: desierto lejano muy oscuro con bruma.
        horizon.lerp(Vec3::new(0.030, 0.028, 0.045), smoothstep(0.0, 0.15, -up))
    };
    // Resplandor verde-azulado en el horizonte.
    c += Vec3::new(0.02, 0.05, 0.06) * (1.0 - smoothstep(0.0, 0.18, up.abs()));

    // Banda tenue tipo vía láctea.
    let band_axis = Vec3::new(0.3, 0.55, 0.78).normalized();
    let band = 1.0 - smoothstep(0.0, 0.28, d.dot(band_axis).abs());
    let neb = fbm_dir(d * 3.0, 50);
    c += Vec3::new(0.05, 0.04, 0.09) * band * neb;

    // Estrellas: celdas en una rejilla sobre la dirección.
    if up > -0.05 {
        let s = d * 420.0;
        let (ix, iy, iz) = (s.x.floor() as i32, s.y.floor() as i32, s.z.floor() as i32);
        let hh = hash_u32((ix as u32).wrapping_mul(73_856_093) ^ (iy as u32).wrapping_mul(19_349_663) ^ (iz as u32).wrapping_mul(83_492_791));
        let r = (hh & 0xFFFF) as f32 / 65535.0;
        if r > 0.9965 {
            let b = ((hh >> 16) & 0xFF) as f32 / 255.0;
            let tint = if b > 0.7 { Vec3::new(0.8, 0.85, 1.0) } else { Vec3::new(1.0, 0.95, 0.85) };
            c += tint * (0.15 + 0.55 * b * b) * (1.0 + band);
        }
    }

    // Luna creciente.
    let m = MOON_DIR.normalized();
    let ang = d.dot(m).clamp(-1.0, 1.0).acos();
    let radius = 0.115;
    // Halo.
    c += Vec3::new(0.25, 0.30, 0.42) * (0.18 * (-ang / 0.25).exp() + 0.05 * (-ang / 0.8).exp());
    if ang < radius * 1.02 {
        // "Sombra" de la creciente: un disco desplazado que tapa parte de la luna.
        let right = m.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
        let shadow_c = (m + right * (radius * 0.55) + Vec3::new(0.0, radius * 0.25, 0.0)).normalized();
        let a2 = d.dot(shadow_c).clamp(-1.0, 1.0).acos();
        let lit = smoothstep(radius * 0.93, radius * 1.02, a2) * (1.0 - smoothstep(radius * 0.97, radius * 1.02, ang));
        let maria = fbm_dir(d * 40.0, 70);
        let moon = Vec3::new(0.98, 0.97, 0.92) * (0.82 + 0.18 * maria);
        c = c.lerp(moon, lit);
    }

    // Garganta: grieta roja irregular.
    // Detrás del espectador: solo se ve reflejada en el acero y la obsidiana.
    let g_center = GARGANTA_DIR.normalized();
    let g_right = g_center.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
    let g_up = g_right.cross(g_center);
    let (lx, ly) = (d.dot(g_right), d.dot(g_up));
    if d.dot(g_center) > 0.85 && lx.abs() < 0.24 {
        let jag = (lx * 60.0).sin() * 0.006 + (lx * 23.0).sin() * 0.01;
        let width = 0.02 * (1.0 - (lx / 0.24).powi(2));
        let dist = (ly - jag - lx * 0.35).abs();
        let core = 1.0 - smoothstep(width * 0.3, width, dist);
        let glow = (-dist / 0.04).exp() * (1.0 - (lx / 0.24).powi(2));
        c += Vec3::new(0.95, 0.10, 0.12) * glow * 0.6;
        c = c.lerp(Vec3::new(0.02, 0.0, 0.01), core);
    }
    c
}

fn fbm_dir(p: Vec3, seed: u32) -> f32 {
    // Ruido de valor 3D simple (para nebulosa y manchas de la luna).
    let n3 = |q: Vec3, s: u32| {
        let (xi, yi, zi) = (q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32);
        let (fx, fy, fz) = (q.x - xi as f32, q.y - yi as f32, q.z - zi as f32);
        let h = |a: i32, b: i32, c: i32| {
            (hash_u32((a as u32).wrapping_mul(73_856_093) ^ (b as u32).wrapping_mul(19_349_663) ^ (c as u32).wrapping_mul(83_492_791) ^ s)
                & 0xFFFF) as f32
                / 65535.0
        };
        let (sx, sy, sz) = (smoothstep(0.0, 1.0, fx), smoothstep(0.0, 1.0, fy), smoothstep(0.0, 1.0, fz));
        let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
        l(
            l(l(h(xi, yi, zi), h(xi + 1, yi, zi), sx), l(h(xi, yi + 1, zi), h(xi + 1, yi + 1, zi), sx), sy),
            l(l(h(xi, yi, zi + 1), h(xi + 1, yi, zi + 1), sx), l(h(xi, yi + 1, zi + 1), h(xi + 1, yi + 1, zi + 1), sx), sy),
            sz,
        )
    };
    let (mut s, mut a, mut q) = (0.0, 0.5, p);
    for o in 0..4 {
        s += a * n3(q, seed + o);
        a *= 0.5;
        q = q * 2.03;
    }
    s / 0.9375
}

fn gen_skybox() {
    for (f, name) in SKY_FACES.iter().enumerate() {
        let mut img = Rgb8::new(SKY, SKY);
        for y in 0..SKY {
            for x in 0..SKY {
                let u = (x as f32 + 0.5) / SKY as f32;
                let v = 1.0 - (y as f32 + 0.5) / SKY as f32;
                let d = face_dir(f, u, v).normalized();
                // El cielo se guarda en sRGB (gamma de pantalla).
                let c = sky_color(d);
                let g = |x: f32| x.max(0.0).powf(1.0 / 2.2);
                img.put(x, y, to8(Vec3::new(g(c.x), g(c.y), g(c.z))));
            }
        }
        let path = Path::new("assets/skybox").join(format!("{name}.bmp"));
        save_bmp(&path, &img).expect("no se pudo escribir el skybox");
        println!("  {}", path.display());
    }
}

/// Texturas solo para las escenas de diagnóstico (--diag).
fn gen_diag_textures() {
    let save = |name: &str, img: &Rgb8| {
        let path = Path::new("assets/diag").join(format!("{name}.bmp"));
        save_bmp(&path, img).expect("no se pudo escribir");
        println!("  {}", path.display());
    };
    // Tablero de ajedrez de alto contraste.
    let mut img = Rgb8::new(TEX, TEX);
    for y in 0..TEX {
        for x in 0..TEX {
            let on = ((x / 16) + (y / 16)) % 2 == 0;
            img.put(x, y, if on { [230, 230, 230] } else { [30, 30, 40] });
        }
    }
    save("checker", &img);
    // Prueba de UV: degradado rojo en u, verde en v, y una letra "F" asimétrica.
    let mut img = Rgb8::new(TEX, TEX);
    for y in 0..TEX {
        for x in 0..TEX {
            let (u, v) = (x as f32 / TEX as f32, 1.0 - y as f32 / TEX as f32);
            let (gx, gy) = (x * 8 / TEX, y * 8 / TEX); // rejilla 8x8, y hacia abajo
            let f = (gx == 2 && (1..=6).contains(&gy)) || (gy == 1 && (2..=5).contains(&gx)) || (gy == 3 && (2..=4).contains(&gx));
            let c = if f { [255, 255, 255] } else { [(u * 220.0) as u8 + 20, (v * 220.0) as u8 + 20, 60] };
            img.put(x, y, c);
        }
    }
    save("uvtest", &img);
}

fn main() {
    println!("Generando texturas...");
    gen_block_textures();
    println!("Generando texturas de diagnóstico...");
    gen_diag_textures();
    println!("Generando skybox...");
    gen_skybox();
    println!("Listo.");
}
