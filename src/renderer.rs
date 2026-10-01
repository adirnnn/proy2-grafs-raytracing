//! raytracer recursivo (estilo whitted). este es el corazón del programa: por
//! cada píxel lanzamos rayos desde la cámara, buscamos con la bvh qué objeto tocan
//! y calculamos el color en ese punto. lo que hace este módulo:
//!
//! 1. iluminación local phong (difuso más especular) con sombras duras o suaves por luz.
//! 2. reflexión y refracción recursivas con fresnel (aproximación de schlick) y
//!    reflexión interna total.
//! 3. skybox para todo rayo que no choca (primarios, reflejados, refractados).
//! 4. niebla exponencial con la distancia.
//! 5. antialiasing por jitter subpíxel y acumulación progresiva; con semilla fija el
//!    resultado es determinista.
//! 6. render en paralelo por filas y, al final, mapeo de tonos y conversión a srgb.
//!
//! el diorama puede girar: en lugar de mover los objetos, llevamos cada rayo al
//! espacio del diorama con la rotación inversa, así la bvh no se reconstruye.

use crate::camera::Camera;
use crate::geometry::Ray;
use crate::material::Material;
use crate::math::{Mat3, Rng, Vec3};
use crate::scene::{Light, Scene, PIVOT};
use std::sync::atomic::{AtomicUsize, Ordering};

/// desplazamiento pequeño para evitar la autointersección ("acné" de sombras):
/// al lanzar un rayo secundario lo sacamos un poquito de la superficie.
const EPS: f32 = 2e-3;
/// distancia máxima que consideramos; más allá de esto es "infinito" (cielo).
const FAR: f32 = 1.0e4;

/// parámetros de calidad del render.
#[derive(Clone, Copy, Debug)]
pub struct Quality {
    /// cantidad máxima de rebotes recursivos (reflexión o refracción) por rayo.
    pub max_depth: u32,
    /// si es verdadero, las luces direccionales con tamaño dan sombras suaves.
    pub soft_shadows: bool,
}

/// estado de un cuadro: rotación del diorama más la cámara.
pub struct View {
    /// rotación que lleva del diorama al mundo.
    pub rot: Mat3,
    /// rotación que lleva del mundo al diorama (la transpuesta de `rot`).
    pub inv: Mat3,
    /// cámara desde la que se ve el cuadro.
    pub camera: Camera,
    /// base ortonormal de la cámara precalculada (para no recalcularla en cada píxel).
    basis: (Vec3, Vec3, Vec3),
    /// luces llevadas al espacio del diorama.
    lights: Vec<Light>,
}

impl View {
    /// arma la vista de un cuadro. `scene` da las luces originales, `yaw` es el ángulo
    /// de giro del diorama alrededor del eje y (en radianes) y `camera` la cámara.
    pub fn new(scene: &Scene, yaw: f32, camera: Camera) -> View {
        let rot = Mat3::rot_y(yaw);
        // como es rotación pura, la inversa es la transpuesta
        let inv = rot.transpose();
        let lights = scene
            .lights
            .iter()
            .map(|l| match *l {
                // la luna es parte del cielo (mundo): su dirección se gira al espacio del diorama.
                Light::Directional { dir, color, spread } => Light::Directional { dir: inv.apply(dir), color, spread },
                // la luz de la cúpula es parte del diorama: gira con él, así que se queda igual.
                p @ Light::Point { .. } => p,
            })
            .collect();
        View { rot, inv, camera, basis: camera.basis(), lights }
    }
}

/// el trazador: junta la escena, la vista del cuadro actual y la calidad.
/// solo guarda referencias, así que es barato de crear y se puede compartir entre hilos.
pub struct Tracer<'a> {
    /// escena con objetos, materiales, texturas, bvh, cielo y niebla.
    pub scene: &'a Scene,
    /// vista del cuadro (rotación del diorama, cámara y luces transformadas).
    pub view: &'a View,
    /// parámetros de calidad.
    pub q: Quality,
}

impl<'a> Tracer<'a> {
    /// color del cielo en la dirección `dir_obj`, que viene en espacio del diorama.
    /// el skybox está fijo en el mundo, por eso primero rotamos la dirección de vuelta al mundo.
    fn sky(&self, dir_obj: Vec3) -> Vec3 {
        self.scene.skybox.sample(self.view.rot.apply(dir_obj))
    }

    /// luz que llega desde `p` hacia una luz, atenuada por objetos transparentes.
    /// `dir` es la dirección hacia la luz y `dist` la distancia hasta ella.
    /// devuelve un factor rgb: uno si no hay nada en medio, cero si está totalmente
    /// bloqueada, o algo intermedio y tintado si en medio hay cristales.
    fn transmittance(&self, p: Vec3, dir: Vec3, dist: f32) -> Vec3 {
        let mut ray = Ray { origin: p, dir };
        // cuánto camino falta para llegar a la luz
        let mut left = dist;
        // atenuación acumulada, empieza sin atenuar
        let mut att = Vec3::ONE;
        // atravesamos como máximo 16 superficies transparentes
        for _ in 0..16 {
            // si no chocamos con nada antes de la luz, la luz llega con la atenuación que llevamos
            let Some(h) = self.scene.bvh.intersect(&self.scene.objects, &ray, EPS, left) else {
                return att;
            };
            let m = &self.scene.materials[h.material as usize];
            // un objeto opaco bloquea la luz por completo: sombra
            if m.transparency <= 0.0 {
                return Vec3::ZERO;
            }
            // sombra "tintada": los cristales dejan pasar parte de la luz (sin cáusticas).
            att = att.mul(m.transmit_tint) * m.transparency;
            // si ya casi no pasa luz, cortamos para ahorrar trabajo
            if att.max_component() < 0.02 {
                return Vec3::ZERO;
            }
            // descontamos lo recorrido y seguimos desde un poquito después del impacto
            left -= h.t + EPS;
            ray.origin = h.point + dir * EPS;
        }
        // demasiadas superficies transparentes seguidas: se considera bloqueada.
        Vec3::ZERO
    }

    /// traza un rayo y devuelve el color que ve.
    /// `ray` está en espacio del diorama, `depth` es el nivel de recursión actual,
    /// `weight` es cuánto aporta este rayo al píxel final (para cortar ramas que ya no
    /// importan) y `rng` es el generador aleatorio del píxel.
    pub fn trace(&self, ray: &Ray, depth: u32, weight: f32, rng: &mut Rng) -> Vec3 {
        // si el rayo no toca nada, devolvemos el color del cielo
        let Some(hit) = self.scene.bvh.intersect(&self.scene.objects, ray, EPS, FAR) else {
            return self.sky(ray.dir);
        };
        let c = self.shade(ray, &hit, depth, weight, rng);
        // niebla exponencial con la distancia: oculta el horizonte del suelo y da escala.
        if self.scene.fog_density > 0.0 {
            // f es la fracción de niebla: uno menos e a la menos (distancia por densidad)
            let f = 1.0 - (-hit.t * self.scene.fog_density).exp();
            // mezclamos el color de la superficie con el de la niebla según f
            return c.lerp(self.scene.fog_color, f);
        }
        c
    }

    /// calcula el color en el punto de impacto `hit` del rayo `ray`.
    /// suma iluminación local (phong con sombras), emisión, reflexión y refracción.
    /// `depth`, `weight` y `rng` tienen el mismo significado que en `trace`.
    fn shade(&self, ray: &Ray, hit: &crate::geometry::Hit, depth: u32, weight: f32, rng: &mut Rng) -> Vec3 {
        let hit = *hit;
        let m: &Material = &self.scene.materials[hit.material as usize];
        // color de la textura en (u, v), escalando por la repetición propia del material
        let tex = self.scene.textures[hit.material as usize].sample(hit.u * m.uv_scale, hit.v * m.uv_scale, true);

        let d = ray.dir;
        // si la dirección y la normal saliente apuntan al mismo lado, el rayo viene desde adentro del objeto
        let inside = d.dot(hit.normal) > 0.0;
        // normal orientada hacia el lado de donde viene el rayo.
        let n = if inside { -hit.normal } else { hit.normal };
        // punto apenas afuera de la superficie (del lado del rayo) para lanzar sombras y reflejos
        let p_out = hit.point + n * EPS;

        // iluminación local (phong)
        // la energía que no se va en reflejo ni en transparencia es la que se ve como difuso
        let k_local = (1.0 - m.reflectivity - m.transparency).max(0.0);
        // empezamos con la luz ambiental (depende de la orientación de la normal) por la textura
        let mut diffuse = self.scene.ambient(n).mul(tex) * m.albedo;
        let mut spec = Vec3::ZERO;
        // dirección hacia el observador (opuesta al rayo)
        let view_dir = -d;
        // la cara interior de un cristal no recibe luz directa (vendría desde atrás del cristal).
        let lights: &[Light] = if inside && m.transparency > 0.0 { &[] } else { &self.view.lights };
        for light in lights {
            // para cada luz sacamos dirección hacia ella, color que llega y distancia
            let (l_dir, l_color, l_dist) = match *light {
                Light::Directional { dir, color, spread } => {
                    let mut ld = dir;
                    if self.q.soft_shadows && spread > 0.0 {
                        // luna con tamaño: muestreamos una dirección dentro de un cono pequeño.
                        // el jitter es un vector aleatorio en un cubo centrado en cero
                        let j = Vec3::new(rng.next_f32() - 0.5, rng.next_f32() - 0.5, rng.next_f32() - 0.5);
                        // desviamos un poco la dirección según spread y renormalizamos;
                        // promediando muchas muestras queda una penumbra suave
                        ld = (dir + j * (2.0 * spread)).normalized();
                    }
                    // una luz direccional está "infinitamente" lejos
                    (ld, color, FAR)
                }
                Light::Point { pos, color, radius } => {
                    let to = pos - hit.point;
                    let dist = to.length();
                    // alcance máximo: más allá de 6 radios la luz se considera nula.
                    if dist > 6.0 * radius {
                        continue;
                    }
                    // caída suave con la distancia: uno entre (1 más (dist entre radio) al cuadrado)
                    let fall = 1.0 / (1.0 + (dist / radius) * (dist / radius));
                    // to entre dist es la dirección unitaria hacia la luz
                    (to / dist, color * fall, dist)
                }
            };
            // término de lambert: coseno entre la normal y la dirección de la luz
            let ndl = n.dot(l_dir);
            // sin aporte visible (de espaldas o una luz puntual muy lejana): no lanzamos sombra.
            if ndl <= 0.0 || l_color.max_component() < 0.01 {
                continue;
            }
            // rayo de sombra: cuánta luz llega realmente hasta aquí
            let vis = self.transmittance(p_out, l_dir, l_dist);
            if vis.max_component() <= 0.0 {
                continue;
            }
            let lc = l_color.mul(vis);
            // difuso: luz por textura por albedo por coseno
            diffuse += lc.mul(tex) * (m.albedo * ndl);
            if m.specular > 0.0 {
                // especular de phong: reflejamos la dirección de la luz sobre la normal
                // y vemos qué tanto se alinea con la dirección hacia la cámara
                let r = (-l_dir).reflect(n);
                // elevar a shininess hace el brillo más pequeño y concentrado mientras más grande sea
                let s = r.dot(view_dir).max(0.0).powf(m.shininess);
                spec += lc * (m.specular * s);
            }
        }
        // color local: difuso pesado por k_local, más especular, más lo que el material emite por sí mismo
        let mut color = diffuse * k_local + spec + tex * m.emission;

        // reflexión y refracción
        // kr y kt son los pesos de reflexión y transmisión; fresnel los va a redistribuir
        let mut kr = m.reflectivity;
        let mut kt = m.transparency;
        let mut refr_dir = None;
        if kt > 0.0 {
            // entrando: del aire al material. saliendo (rayo dentro): del material al aire.
            let (n1, n2) = if inside { (m.ior, 1.0) } else { (1.0, m.ior) };
            match refract(d, n, n1, n2) {
                None => {
                    // reflexión interna total: toda la parte transmitida se refleja.
                    kr += kt;
                    kt = 0.0;
                }
                Some((t_dir, f)) => {
                    // fresnel: una fracción f de lo transmitido en realidad se refleja
                    // (más mientras más rasante sea el ángulo) y el resto sí pasa
                    kr += kt * f;
                    kt *= 1.0 - f;
                    refr_dir = Some(t_dir);
                }
            }
        }
        // solo seguimos rebotando si no llegamos al límite de profundidad
        let can_recurse = depth < self.q.max_depth;
        if kr > 0.0 {
            // dirección espejo del rayo incidente
            let r_dir = d.reflect(n).normalized();
            // peso acumulado de este rayo hijo en el píxel
            let w = weight * kr;
            // si el aporte ya es menor al uno por ciento no vale la pena trazarlo
            let c = if can_recurse && w > 0.01 {
                self.trace(&Ray { origin: p_out, dir: r_dir }, depth + 1, w, rng)
            } else {
                // sin presupuesto de rebotes: aproximamos con el cielo.
                self.sky(r_dir)
            };
            // los metales tiñen su reflejo con su textura; el cristal no.
            let tint = if m.transparency > 0.0 { Vec3::ONE } else { Vec3::ONE.lerp(tex, 0.35) };
            color += c.mul(tint) * kr;
        }
        if let Some(t_dir) = refr_dir {
            let w = weight * kt;
            let c = if can_recurse && w > 0.01 {
                // el rayo refractado nace al otro lado de la superficie, por eso restamos la normal
                let origin = hit.point - n * EPS;
                self.trace(&Ray { origin, dir: t_dir }, depth + 1, w, rng)
            } else {
                self.sky(t_dir)
            };
            // la luz que atraviesa toma el color del cristal y un poco el de su textura
            let tint = m.transmit_tint.mul(Vec3::ONE.lerp(tex, 0.5));
            color += c.mul(tint) * kt;
        }
        color
    }

    /// color de un píxel promediando `spp` muestras con jitter.
    /// (`x`, `y`) es el píxel, `w` y `h` el tamaño de la imagen y `sample_base` el número de
    /// la primera muestra, que hace la secuencia determinista entre pasadas progresivas.
    pub fn pixel(&self, x: usize, y: usize, w: usize, h: usize, spp: u32, sample_base: u32) -> Vec3 {
        let mut acc = Vec3::ZERO;
        for s in 0..spp {
            // índice global de la muestra (sumando las pasadas anteriores)
            let si = sample_base + s;
            // semilla única por píxel y por muestra: mezclamos x, y y el índice con primos distintos
            let mut rng = Rng::new((y as u32).wrapping_mul(9781) ^ (x as u32).wrapping_mul(6271) ^ si.wrapping_mul(26699));
            // la primera muestra va al centro del píxel; las demás a una posición aleatoria dentro de él
            let (jx, jy) = if si == 0 { (0.5, 0.5) } else { (rng.next_f32(), rng.next_f32()) };
            // rayo primario en espacio del mundo
            let wr = self.view.camera.ray(&self.view.basis, x as f32 + jx, y as f32 + jy, w as f32, h as f32);
            // llevamos el rayo al espacio del diorama (rotación del diorama).
            // el diorama gira alrededor de `PIVOT`, el centro de las noches, así que
            // el origen se rota alrededor de ese punto y la dirección solo se rota.
            let r = Ray { origin: self.view.inv.apply(wr.origin - PIVOT) + PIVOT, dir: self.view.inv.apply(wr.dir) };
            acc += self.trace(&r, 0, 1.0, &mut rng);
        }
        // promedio de las muestras
        acc / spp as f32
    }
}

/// refracción por la ley de snell. `d` es la dirección incidente (normalizada), `n` la normal
/// del lado de donde viene el rayo (d punto n menor que cero), y la luz pasa del índice `n1` al `n2`.
/// devuelve (dirección transmitida, reflectancia de fresnel schlick) o `None` si hay
/// reflexión interna total.
pub fn refract(d: Vec3, n: Vec3, n1: f32, n2: f32) -> Option<(Vec3, f32)> {
    // razón de índices
    let eta = n1 / n2;
    // coseno del ángulo de incidencia (d apunta hacia la superficie, por eso lo negamos)
    let cos_i = (-d.dot(n)).clamp(0.0, 1.0);
    // snell: seno del ángulo transmitido es eta por seno del incidente; lo usamos al cuadrado
    // y escribimos seno al cuadrado como uno menos coseno al cuadrado
    let sin2_t = eta * eta * (1.0 - cos_i * cos_i);
    // si el seno al cuadrado pasa de uno no existe ángulo transmitido: reflexión interna total
    if sin2_t > 1.0 {
        return None;
    }
    let cos_t = (1.0 - sin2_t).sqrt();
    // schlick usa el coseno del lado del medio menos denso.
    // r0 es la reflectancia con incidencia perpendicular: ((n1 menos n2) entre (n1 más n2)) al cuadrado
    let r0 = ((n1 - n2) / (n1 + n2)).powi(2);
    let c = if n1 > n2 { cos_t } else { cos_i };
    // aproximación de schlick: r0 más (1 menos r0) por (1 menos coseno) a la quinta
    let f = r0 + (1.0 - r0) * (1.0 - c).powi(5);
    // dirección transmitida: eta por d más n por (eta por cos_i menos cos_t)
    Some(((d * eta + n * (eta * cos_i - cos_t)).normalized(), f))
}

/// renderiza `spp` muestras por píxel y las suma en `accum` (acumulación progresiva).
/// `tracer` es el trazador del cuadro, `w` y `h` el tamaño, `accum` el búfer de sumas
/// (uno por píxel, fila por fila) y `sample_base` el índice de la primera muestra de esta pasada.
/// paraleliza por filas con hilos de la biblioteca estándar.
pub fn render_accumulate(tracer: &Tracer, w: usize, h: usize, accum: &mut [Vec3], spp: u32, sample_base: u32) {
    // usamos tantos hilos como núcleos reporte el sistema (o 4 si no se puede saber)
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    // contador atómico compartido: cada hilo toma la siguiente fila libre
    let next_row = AtomicUsize::new(0);
    // partimos el búfer en filas y envolvemos cada una en un mutex para poder escribirla desde un hilo
    let rows: Vec<std::sync::Mutex<&mut [Vec3]>> = accum.chunks_mut(w).map(std::sync::Mutex::new).collect();
    // thread::scope nos deja prestar tracer y rows a los hilos y espera a que todos terminen
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                // fetch_add devuelve el valor anterior y lo incrementa, así ninguna fila se repite
                let y = next_row.fetch_add(1, Ordering::Relaxed);
                if y >= h {
                    break;
                }
                let mut row = rows[y].lock().unwrap();
                for x in 0..w {
                    // pixel devuelve el promedio, lo multiplicamos por spp para guardar la suma
                    row[x] += tracer.pixel(x, y, w, h, spp, sample_base) * spp as f32;
                }
            });
        }
    });
}

/// mapeo de tonos (aces aproximado) más exposición más viñeta, y conversión a srgb de 8 bits.
/// es la única conversión de lineal a srgb del programa.
/// `accum` tiene las sumas, `samples` cuántas muestras lleva cada píxel, `w` y `h` el tamaño,
/// `exposure` el multiplicador de brillo y `out` el búfer rgb de salida (3 bytes por píxel).
pub fn tonemap(accum: &[Vec3], samples: u32, w: usize, h: usize, exposure: f32, out: &mut [u8]) {
    // dividir entre las muestras da el promedio; de una vez aplicamos la exposición
    let inv = exposure / samples.max(1) as f32;
    for y in 0..h {
        for x in 0..w {
            // posición del píxel relativa al centro de la imagen
            let dx = x as f32 / w as f32 - 0.5;
            let dy = y as f32 / h as f32 - 0.5;
            // viñeta: oscurecemos según la distancia al centro al cuadrado (hasta 35 por ciento en las esquinas)
            let vig = 1.0 - 0.35 * (dx * dx + dy * dy) * 2.0;
            let c = accum[y * w + x] * (inv * vig);
            // índice del primer byte del píxel en el búfer rgb
            let i = (y * w + x) * 3;
            out[i] = to_srgb8(aces(c.x));
            out[i + 1] = to_srgb8(aces(c.y));
            out[i + 2] = to_srgb8(aces(c.z));
        }
    }
}

/// curva de tonos aces aproximada (ajuste racional de narkowicz).
/// comprime valores altos de forma suave para que los brillos no se "quemen" y deja el resultado entre cero y uno.
#[inline]
fn aces(x: f32) -> f32 {
    let x = x.max(0.0);
    ((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14)).clamp(0.0, 1.0)
}

/// convierte un valor lineal a srgb y lo cuantiza a un byte de cero a 255.
#[inline]
pub fn to_srgb8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    // función de transferencia srgb: tramo lineal cerca del negro y curva de potencia 1 entre 2.4 en el resto
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    // sumamos 0.5 para redondear al entero más cercano en vez de truncar
    (s * 255.0 + 0.5) as u8
}
