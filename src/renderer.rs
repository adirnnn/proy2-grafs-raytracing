//! Raytracer recursivo (estilo Whitted):
//! - Phong (difuso + especular) con sombras duras/suaves por luz.
//! - Reflexión y refracción recursivas con Fresnel (Schlick) y reflexión interna total.
//! - Skybox para todo rayo que no choca (primarios, reflejados, refractados).
//! - Antialiasing por jitter subpíxel y acumulación progresiva; semilla fija => determinista.

use crate::camera::Camera;
use crate::geometry::Ray;
use crate::material::Material;
use crate::math::{Mat3, Rng, Vec3};
use crate::scene::{Light, Scene};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Desplazamiento para evitar auto-intersección ("acné" de sombras).
const EPS: f32 = 2e-3;
const FAR: f32 = 1.0e4;

#[derive(Clone, Copy, Debug)]
pub struct Quality {
    pub max_depth: u32,
    pub soft_shadows: bool,
}

/// Estado de un cuadro: rotación del diorama + cámara.
pub struct View {
    /// Diorama -> mundo.
    pub rot: Mat3,
    /// Mundo -> diorama.
    pub inv: Mat3,
    pub camera: Camera,
    basis: (Vec3, Vec3, Vec3),
    /// Luces llevadas al espacio del diorama.
    lights: Vec<Light>,
}

impl View {
    pub fn new(scene: &Scene, yaw: f32, camera: Camera) -> View {
        let rot = Mat3::rot_y(yaw);
        let inv = rot.transpose();
        let lights = scene
            .lights
            .iter()
            .map(|l| match *l {
                // La luna es parte del cielo (mundo): su dirección se gira al espacio del diorama.
                Light::Directional { dir, color, spread } => Light::Directional { dir: inv.apply(dir), color, spread },
                // La luz de la cúpula es parte del diorama: gira con él.
                p @ Light::Point { .. } => p,
            })
            .collect();
        View { rot, inv, camera, basis: camera.basis(), lights }
    }
}

pub struct Tracer<'a> {
    pub scene: &'a Scene,
    pub view: &'a View,
    pub q: Quality,
}

impl<'a> Tracer<'a> {
    fn sky(&self, dir_obj: Vec3) -> Vec3 {
        self.scene.skybox.sample(self.view.rot.apply(dir_obj))
    }

    /// Luz que llega desde `p` hacia una luz, atenuada por objetos transparentes.
    fn transmittance(&self, p: Vec3, dir: Vec3, dist: f32) -> Vec3 {
        let mut ray = Ray { origin: p, dir };
        let mut left = dist;
        let mut att = Vec3::ONE;
        for _ in 0..16 {
            let Some(h) = self.scene.bvh.intersect(&self.scene.objects, &ray, EPS, left) else {
                return att;
            };
            let m = &self.scene.materials[h.material as usize];
            if m.transparency <= 0.0 {
                return Vec3::ZERO;
            }
            // Sombra "tintada": los cristales dejan pasar parte de la luz (sin cáusticas).
            att = att.mul(m.transmit_tint) * m.transparency;
            if att.max_component() < 0.02 {
                return Vec3::ZERO;
            }
            left -= h.t + EPS;
            ray.origin = h.point + dir * EPS;
        }
        // Demasiadas superficies transparentes seguidas: se considera bloqueada.
        Vec3::ZERO
    }

    pub fn trace(&self, ray: &Ray, depth: u32, weight: f32, rng: &mut Rng) -> Vec3 {
        let Some(hit) = self.scene.bvh.intersect(&self.scene.objects, ray, EPS, FAR) else {
            return self.sky(ray.dir);
        };
        let m: &Material = &self.scene.materials[hit.material as usize];
        let tex = self.scene.textures[hit.material as usize].sample(hit.u * m.uv_scale, hit.v * m.uv_scale, true);

        let d = ray.dir;
        let inside = d.dot(hit.normal) > 0.0;
        // Normal orientada hacia el lado de donde viene el rayo.
        let n = if inside { -hit.normal } else { hit.normal };
        let p_out = hit.point + n * EPS;

        // --- Iluminación local (Phong) ---
        let k_local = (1.0 - m.reflectivity - m.transparency).max(0.0);
        let mut diffuse = self.scene.ambient(n).mul(tex) * m.albedo;
        let mut spec = Vec3::ZERO;
        let view_dir = -d;
        // La cara interior de un cristal no recibe luz directa (vendría desde atrás del cristal).
        let lights: &[Light] = if inside && m.transparency > 0.0 { &[] } else { &self.view.lights };
        for light in lights {
            let (l_dir, l_color, l_dist) = match *light {
                Light::Directional { dir, color, spread } => {
                    let mut ld = dir;
                    if self.q.soft_shadows && spread > 0.0 {
                        // Luna con tamaño: muestreamos una dirección dentro de un cono pequeño.
                        let j = Vec3::new(rng.next_f32() - 0.5, rng.next_f32() - 0.5, rng.next_f32() - 0.5);
                        ld = (dir + j * (2.0 * spread)).normalized();
                    }
                    (ld, color, FAR)
                }
                Light::Point { pos, color, radius } => {
                    let to = pos - hit.point;
                    let dist = to.length();
                    // Caída suave con la distancia.
                    let fall = 1.0 / (1.0 + (dist / radius) * (dist / radius));
                    (to / dist, color * fall, dist)
                }
            };
            let ndl = n.dot(l_dir);
            if ndl <= 0.0 {
                continue;
            }
            let vis = self.transmittance(p_out, l_dir, l_dist);
            if vis.max_component() <= 0.0 {
                continue;
            }
            let lc = l_color.mul(vis);
            diffuse += lc.mul(tex) * (m.albedo * ndl);
            if m.specular > 0.0 {
                let r = (-l_dir).reflect(n);
                let s = r.dot(view_dir).max(0.0).powf(m.shininess);
                spec += lc * (m.specular * s);
            }
        }
        let mut color = diffuse * k_local + spec + tex * m.emission;

        // --- Reflexión y refracción ---
        let mut kr = m.reflectivity;
        let mut kt = m.transparency;
        let mut refr_dir = None;
        if kt > 0.0 {
            // Entrando: aire -> material. Saliendo (rayo dentro): material -> aire.
            let (n1, n2) = if inside { (m.ior, 1.0) } else { (1.0, m.ior) };
            match refract(d, n, n1, n2) {
                None => {
                    // Reflexión interna total: toda la parte transmitida se refleja.
                    kr += kt;
                    kt = 0.0;
                }
                Some((t_dir, f)) => {
                    kr += kt * f;
                    kt *= 1.0 - f;
                    refr_dir = Some(t_dir);
                }
            }
        }
        let can_recurse = depth < self.q.max_depth;
        if kr > 0.0 {
            let r_dir = d.reflect(n).normalized();
            let w = weight * kr;
            let c = if can_recurse && w > 0.01 {
                self.trace(&Ray { origin: p_out, dir: r_dir }, depth + 1, w, rng)
            } else {
                // Sin presupuesto de rebotes: aproximamos con el cielo.
                self.sky(r_dir)
            };
            // Los metales tiñen su reflejo con su textura; el cristal no.
            let tint = if m.transparency > 0.0 { Vec3::ONE } else { Vec3::ONE.lerp(tex, 0.35) };
            color += c.mul(tint) * kr;
        }
        if let Some(t_dir) = refr_dir {
            let w = weight * kt;
            let c = if can_recurse && w > 0.01 {
                let origin = hit.point - n * EPS; // al otro lado de la superficie
                self.trace(&Ray { origin, dir: t_dir }, depth + 1, w, rng)
            } else {
                self.sky(t_dir)
            };
            let tint = m.transmit_tint.mul(Vec3::ONE.lerp(tex, 0.5));
            color += c.mul(tint) * kt;
        }
        color
    }

    /// Color de un píxel con `spp` muestras jitter; `sample_base` hace la secuencia determinista.
    pub fn pixel(&self, x: usize, y: usize, w: usize, h: usize, spp: u32, sample_base: u32) -> Vec3 {
        let mut acc = Vec3::ZERO;
        for s in 0..spp {
            let si = sample_base + s;
            let mut rng = Rng::new((y as u32).wrapping_mul(9781) ^ (x as u32).wrapping_mul(6271) ^ si.wrapping_mul(26699));
            let (jx, jy) = if si == 0 { (0.5, 0.5) } else { (rng.next_f32(), rng.next_f32()) };
            let wr = self.view.camera.ray(&self.view.basis, x as f32 + jx, y as f32 + jy, w as f32, h as f32);
            // Llevamos el rayo al espacio del diorama (rotación del diorama).
            let r = Ray { origin: self.view.inv.apply(wr.origin), dir: self.view.inv.apply(wr.dir) };
            acc += self.trace(&r, 0, 1.0, &mut rng);
        }
        acc / spp as f32
    }
}

/// Refracción por la ley de Snell. `d` = dirección incidente (normalizada), `n` = normal
/// del lado de donde viene el rayo (d·n < 0), `n1` -> `n2` índices de refracción.
/// Devuelve (dirección transmitida, reflectancia de Fresnel-Schlick) o `None` si hay
/// reflexión interna total.
pub fn refract(d: Vec3, n: Vec3, n1: f32, n2: f32) -> Option<(Vec3, f32)> {
    let eta = n1 / n2;
    let cos_i = (-d.dot(n)).clamp(0.0, 1.0);
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
    if sin2_t > 1.0 {
        return None;
    }
    let cos_t = (1.0 - sin2_t).sqrt();
    // Schlick usa el coseno del lado del medio MENOS denso.
    let r0 = ((n1 - n2) / (n1 + n2)).powi(2);
    let c = if n1 > n2 { cos_t } else { cos_i };
    let f = r0 + (1.0 - r0) * (1.0 - c).powi(5);
    Some(((d * eta + n * (eta * cos_i - cos_t)).normalized(), f))
}

/// Renderiza `spp` muestras por píxel y las SUMA en `accum` (acumulación progresiva).
/// Paraleliza por filas con hilos de la biblioteca estándar.
pub fn render_accumulate(tracer: &Tracer, w: usize, h: usize, accum: &mut [Vec3], spp: u32, sample_base: u32) {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let next_row = AtomicUsize::new(0);
    let rows: Vec<std::sync::Mutex<&mut [Vec3]>> = accum.chunks_mut(w).map(std::sync::Mutex::new).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let y = next_row.fetch_add(1, Ordering::Relaxed);
                if y >= h {
                    break;
                }
                let mut row = rows[y].lock().unwrap();
                for x in 0..w {
                    row[x] += tracer.pixel(x, y, w, h, spp, sample_base) * spp as f32;
                }
            });
        }
    });
}

/// Mapeo de tonos (ACES aproximado) + exposición + viñeta, y conversión a sRGB de 8 bits.
/// Es la ÚNICA conversión lineal->sRGB del programa.
pub fn tonemap(accum: &[Vec3], samples: u32, w: usize, h: usize, exposure: f32, out: &mut [u8]) {
    let inv = exposure / samples.max(1) as f32;
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 / w as f32 - 0.5;
            let dy = y as f32 / h as f32 - 0.5;
            let vig = 1.0 - 0.35 * (dx * dx + dy * dy) * 2.0;
            let c = accum[y * w + x] * (inv * vig);
            let i = (y * w + x) * 3;
            out[i] = to_srgb8(aces(c.x));
            out[i + 1] = to_srgb8(aces(c.y));
            out[i + 2] = to_srgb8(aces(c.z));
        }
    }
}

#[inline]
fn aces(x: f32) -> f32 {
    let x = x.max(0.0);
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

#[inline]
pub fn to_srgb8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}
