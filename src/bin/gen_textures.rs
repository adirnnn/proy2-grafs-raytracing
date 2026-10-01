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
    // 1. Arena: blanca grisácea, granos y pequeñas piedras (pixel art 16x16 por bloque).
    make("sand", |u, v, x, y| {
        let (px, py) = px16(x, y);
        let n = h01(px, py, 1);
        let soft = fbm(u, v, 4, 3, 5);
        let base = Vec3::new(0.80, 0.80, 0.79) * (0.88 + 0.10 * n + 0.06 * soft);
        if n > 0.94 {
            Vec3::new(0.55, 0.56, 0.57)
        } else if n < 0.05 {
            Vec3::new(0.93, 0.93, 0.92)
        } else {
            base
        }
    });
    // 2. Concreto de Las Noches: gris blanco, poros, líneas de vaciado y juntas de bloque.
    make("stone", |u, v, x, y| {
        let n = fbm(u, v, 4, 4, 2);
        let pores = h01((u * 64.0) as i32, (v * 64.0) as i32, 3);
        let pour = ((v * 4.0).fract() < 0.025) as i32 as f32;
        let seam = x < 2 || y < 2;
        let mut c = Vec3::new(0.80, 0.79, 0.76) * (0.82 + 0.16 * n);
        if pores > 0.97 {
            c = c * 0.6;
        }
        c = c * (1.0 - 0.18 * pour);
        if seam { c * 0.78 } else { c }
    });
    // 3. Cuarzo: blanco pálido lechoso con vetas longitudinales y bordes brillantes.
    make("quartz", |u, v, _, _| {
        let streak = fbm(u * 6.0, v * 0.5, 2, 3, 3);
        let edge = 1.0 - smoothstep(0.0, 0.08, u.min(1.0 - u).min(v).min(1.0 - v));
        let c = Vec3::new(0.86, 0.89, 0.90) * (0.80 + 0.22 * streak);
        c.lerp(Vec3::ONE, edge * 0.6)
    });
    // 4. Obsidiana: negro verdoso (como los bloques oscuros de la referencia) con vetas.
    make("obsidian", |u, v, _, _| {
        let n = fbm(u, v, 3, 4, 4);
        let vein = 1.0 - smoothstep(0.0, 0.03, (n - 0.5).abs());
        let base = Vec3::new(0.06, 0.10, 0.10) * (0.8 + 0.4 * fbm(u, v, 8, 2, 8));
        base.lerp(Vec3::new(0.30, 0.45, 0.45), vein * 0.7)
    });
    // 5. Mármol pulido: blanco con vetas grises suaves y losas grandes.
    make("marble", |u, v, x, y| {
        let warp = fbm(u, v, 2, 4, 61);
        let vein = 1.0 - smoothstep(0.0, 0.05, ((u + v * 0.7 + warp * 1.4) * 3.0).fract().min(1.0 - ((u + v * 0.7 + warp * 1.4) * 3.0).fract()));
        let seam = x < 1 || y < 1;
        let c = Vec3::new(0.93, 0.93, 0.92).lerp(Vec3::new(0.55, 0.57, 0.60), vein * 0.6);
        if seam { c * 0.7 } else { c }
    });
    // 6. Cielo falso: azul diurno con nubes (interior de la cúpula).
    make("fake_sky", |u, v, _, _| {
        let cloud = smoothstep(0.52, 0.72, fbm(u, v, 2, 5, 21));
        let sky = Vec3::new(0.36, 0.60, 0.95).lerp(Vec3::new(0.60, 0.80, 1.0), v);
        sky.lerp(Vec3::new(0.97, 0.98, 1.0), cloud)
    });
    // 7. Basalto: gris oscuro, rugoso y pixelado.
    make("basalt", |u, v, x, y| {
        let (px, py) = px16(x, y);
        let n = h01(px, py, 40) * 0.5 + fbm(u, v, 4, 3, 41) * 0.5;
        let seam = x < 2 || y < 2;
        let c = Vec3::new(0.17, 0.18, 0.19) * (0.75 + 0.5 * n);
        if seam { c * 0.6 } else { c }
    });
}

/// Cielo de Hueco Mundo: noche eterna con nubes de tormenta gris oscuro, luna creciente
/// delgada (casi un anillo) con destello, pocas estrellas entre las nubes, y la Garganta
/// roja detrás del espectador. `d` normalizada, en el mundo.
fn sky_color(d: Vec3) -> Vec3 {
    let up = d.y;
    let m = MOON_DIR.normalized();
    let to_moon = d.dot(m).clamp(-1.0, 1.0).acos();
    // Fondo: negro azul verdoso, un poco más claro hacia el horizonte.
    let zenith = Vec3::new(0.010, 0.013, 0.015);
    let horizon = Vec3::new(0.050, 0.058, 0.060);
    let mut c = if up >= 0.0 {
        horizon.lerp(zenith, smoothstep(0.0, 0.5, up))
    } else {
        horizon.lerp(Vec3::new(0.035, 0.038, 0.040), smoothstep(0.0, 0.15, -up))
    };

    // Estrellas tenues (antes de las nubes, que las tapan).
    if up > 0.0 {
        let s = d * 420.0;
        let hh = hash_u32((s.x.floor() as i32 as u32).wrapping_mul(73_856_093) ^ (s.y.floor() as i32 as u32).wrapping_mul(19_349_663) ^ (s.z.floor() as i32 as u32).wrapping_mul(83_492_791));
        if (hh & 0xFFFF) as f32 / 65535.0 > 0.997 {
            c += Vec3::new(0.85, 0.9, 1.0) * (0.12 + 0.3 * ((hh >> 16) & 0xFF) as f32 / 255.0);
        }
    }

    // Nubes de tormenta: capas de ruido sobre la dirección proyectada.
    if up > -0.05 {
        let q = Vec3::new(d.x, 0.0, d.z) / (up.max(0.0) + 0.25);
        let n1 = fbm_dir(q * 1.6 + Vec3::new(3.0, 0.0, 1.0), 50);
        let n2 = fbm_dir(q * 4.0, 51);
        let density = smoothstep(0.38, 0.75, n1 * 0.75 + n2 * 0.25);
        // Las nubes se iluminan cerca de la luna.
        let lit = 0.035 + 0.22 * (-to_moon / 0.35).exp() + 0.05 * (1.0 - smoothstep(0.0, 0.4, up));
        let cloud = Vec3::new(0.85, 0.92, 0.95) * lit * (0.6 + 0.6 * n2);
        c = c.lerp(cloud, density * (1.0 - smoothstep(0.85, 1.0, up)));
    }

    // Halo de la luna.
    c += Vec3::new(0.55, 0.62, 0.68) * (0.25 * (-to_moon / 0.08).exp() + 0.06 * (-to_moon / 0.3).exp());
    // Destello en cruz (como en la referencia).
    let right = m.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
    let upv = right.cross(m);
    let (lx, ly) = (d.dot(right), d.dot(upv));
    if d.dot(m) > 0.95 {
        let star = (-lx.abs() / 0.003).exp() * (-ly.abs() / 0.05).exp() + (-ly.abs() / 0.003).exp() * (-lx.abs() / 0.05).exp();
        c += Vec3::splat(0.9) * star * 0.35;
    }
    // Luna creciente delgada: disco brillante menos un disco casi del mismo tamaño
    // desplazado; queda un anillo fino más grueso abajo a la izquierda.
    let radius = 0.05;
    if to_moon < radius * 1.05 {
        let shadow_c = (m + right * (radius * 0.18) + upv * (radius * 0.22)).normalized();
        let a2 = d.dot(shadow_c).clamp(-1.0, 1.0).acos();
        let disc = 1.0 - smoothstep(radius * 0.97, radius * 1.02, to_moon);
        let lit = disc * smoothstep(radius * 0.80, radius * 0.86, a2);
        // Interior oscuro (la parte en sombra de la luna).
        c = c.lerp(Vec3::new(0.03, 0.035, 0.04), disc * (1.0 - lit) * 0.85);
        c = c.lerp(Vec3::new(1.0, 1.0, 0.98), lit);
    }

    // Garganta: grieta roja irregular (detrás del espectador).
    let g_center = GARGANTA_DIR.normalized();
    let g_right = g_center.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
    let g_up = g_right.cross(g_center);
    let (gx, gy) = (d.dot(g_right), d.dot(g_up));
    if d.dot(g_center) > 0.85 && gx.abs() < 0.24 {
        let jag = (gx * 60.0).sin() * 0.006 + (gx * 23.0).sin() * 0.01;
        let width = 0.02 * (1.0 - (gx / 0.24).powi(2));
        let dist = (gy - jag - gx * 0.35).abs();
        let core = 1.0 - smoothstep(width * 0.3, width, dist);
        let glow = (-dist / 0.04).exp() * (1.0 - (gx / 0.24).powi(2));
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
