//! primitivas geométricas del raytracer: cubos (cajas alineadas a los ejes,
//! opcionalmente rotadas) y esferas, junto con el rayo y el registro de impacto.
//!
//! todo vive en el "espacio del diorama"; el renderer transforma los rayos a este
//! espacio para rotar el diorama sin reconstruir la bvh. aquí resolvemos la
//! intersección exacta de un rayo con cada primitiva y calculamos su normal y sus
//! coordenadas de textura (u, v), que luego usa el sombreado.

use crate::math::{Mat3, Vec3};

/// un rayo: semirrecta que empieza en `origin` y avanza en `dir`.
/// un punto del rayo a distancia t es origin más dir por t.
#[derive(Clone, Copy)]
pub struct Ray {
    /// punto de partida del rayo.
    pub origin: Vec3,
    /// dirección del rayo (normalmente unitaria, así t es una distancia real).
    pub dir: Vec3,
}

/// resultado de una intersección: dónde, a qué distancia y con qué material chocó el rayo.
#[derive(Clone, Copy)]
pub struct Hit {
    /// parámetro del rayo en el punto de impacto (distancia si la dirección es unitaria).
    pub t: f32,
    /// punto de impacto en espacio del diorama.
    pub point: Vec3,
    /// normal geométrica apuntando hacia afuera del objeto.
    pub normal: Vec3,
    /// coordenada horizontal de textura.
    pub u: f32,
    /// coordenada vertical de textura.
    pub v: f32,
    /// índice del material en la tabla de materiales de la escena.
    pub material: u16,
}

/// forma geométrica de un objeto.
#[derive(Clone, Copy)]
pub enum Shape {
    /// caja alineada a los ejes (en el espacio local si `rot` existe), dada por sus esquinas.
    Cube { min: Vec3, max: Vec3 },
    /// esfera dada por su centro y su radio.
    Sphere { center: Vec3, radius: f32 },
}

/// un objeto de la escena: una forma con su material y algunos extras opcionales.
#[derive(Clone, Copy)]
pub struct Object {
    /// la forma (cubo o esfera).
    pub shape: Shape,
    /// índice del material principal.
    pub material: u16,
    /// material alterno para caras cuya normal apunta hacia `alt_center`
    /// (se usa para la cara interior de la cúpula: afuera yeso, adentro "cielo falso").
    pub alt: Option<(u16, Vec3)>,
    /// rotación opcional alrededor de `pivot` (para la espada inclinada, cristales, etc.).
    pub rot: Option<(Mat3, Vec3)>,
    /// escala de uv: cuántas repeticiones de textura por unidad de mundo.
    pub uv_scale: f32,
}

impl Object {
    /// crea un cubo entre las esquinas `min` y `max` con el material dado, sin rotación.
    pub fn cube(min: Vec3, max: Vec3, material: u16) -> Object {
        Object { shape: Shape::Cube { min, max }, material, alt: None, rot: None, uv_scale: 1.0 }
    }
    /// crea una esfera de centro `center` y radio `radius` con el material dado.
    pub fn sphere(center: Vec3, radius: f32, material: u16) -> Object {
        Object { shape: Shape::Sphere { center, radius }, material, alt: None, rot: None, uv_scale: 1.0 }
    }
    /// devuelve el mismo objeto pero rotado con la matriz `m` alrededor del punto `pivot`.
    /// está pensado para encadenar al construir la escena, estilo builder.
    pub fn rotated(mut self, m: Mat3, pivot: Vec3) -> Object {
        self.rot = Some((m, pivot));
        self
    }
    /// devuelve el mismo objeto con otra escala de uv `s` (más repeticiones de textura si es mayor).
    pub fn with_uv_scale(mut self, s: f32) -> Object {
        self.uv_scale = s;
        self
    }

    /// caja envolvente en espacio del diorama (para la bvh).
    /// devuelve la pareja (esquina mínima, esquina máxima).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        // primero la caja en espacio local de la forma
        let (lo, hi) = match self.shape {
            Shape::Cube { min, max } => (min, max),
            // para la esfera basta con el centro más o menos el radio en cada eje
            Shape::Sphere { center, radius } => (center - Vec3::splat(radius), center + Vec3::splat(radius)),
        };
        match self.rot {
            None => (lo, hi),
            Some((m, p)) => {
                // si está rotado, rotamos las 8 esquinas de la caja local y tomamos
                // el mínimo y máximo de todas: eso da una caja alineada que la contiene
                let mut bmin = Vec3::splat(f32::INFINITY);
                let mut bmax = Vec3::splat(f32::NEG_INFINITY);
                for i in 0..8 {
                    // cada bit de i elige lo o hi en un eje, así recorremos las 8 combinaciones
                    let c = Vec3::new(
                        if i & 1 == 0 { lo.x } else { hi.x },
                        if i & 2 == 0 { lo.y } else { hi.y },
                        if i & 4 == 0 { lo.z } else { hi.z },
                    );
                    // rotar alrededor del pivote: trasladamos al origen, rotamos y regresamos
                    let w = m.apply(c - p) + p;
                    bmin = bmin.min(w);
                    bmax = bmax.max(w);
                }
                (bmin, bmax)
            }
        }
    }

    /// interseca el rayo con este objeto dentro del rango (t_min, t_max).
    /// devuelve el impacto con punto y normal ya en espacio del diorama y el material
    /// correcto asignado, o `None` si no hay intersección en ese rango.
    pub fn intersect(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        // si el objeto está rotado, llevamos el rayo a su espacio local con la inversa
        // (transpuesta) de la rotación, y regresamos punto y normal con la rotación.
        // como la rotación no cambia longitudes, el valor de t es el mismo en ambos espacios.
        let local = match self.rot {
            None => *ray,
            Some((m, p)) => {
                let mt = m.transpose();
                // el origen es un punto (se rota alrededor del pivote), la dirección solo se rota
                Ray { origin: mt.apply(ray.origin - p) + p, dir: mt.apply(ray.dir) }
            }
        };
        // el operador ? sale de la función con `None` si la forma no fue tocada
        let mut hit = match self.shape {
            Shape::Cube { min, max } => intersect_cube(&local, min, max, t_min, t_max, self.uv_scale)?,
            Shape::Sphere { center, radius } => intersect_sphere(&local, center, radius, t_min, t_max)?,
        };
        if let Some((m, p)) = self.rot {
            // rotación pura: la normal se transforma con la misma matriz (la inversa transpuesta es m).
            hit.normal = m.apply(hit.normal);
            hit.point = m.apply(hit.point - p) + p;
        }
        hit.material = self.material;
        if let Some((alt, c)) = self.alt {
            // si la normal mira hacia el centro alterno, estamos viendo la cara de adentro
            // (por ejemplo el interior de la cúpula) y usamos el material alterno
            if hit.normal.dot(c - hit.point) > 0.0 {
                hit.material = alt;
            }
        }
        Some(hit)
    }
}

/// intersección rayo contra caja alineada por "slabs".
/// `ray` ya en espacio local, `min` y `max` las esquinas, (t_min, t_max) el rango válido
/// y `uv_scale` la escala de textura. devuelve también el punto de salida si el
/// origen está dentro (necesario para refracción al salir de un cubo de cristal).
pub fn intersect_cube(ray: &Ray, min: Vec3, max: Vec3, t_min: f32, t_max: f32, uv_scale: f32) -> Option<Hit> {
    // t0 es la entrada más tardía y t1 la salida más temprana entre los tres slabs;
    // guardamos además en qué eje ocurrió cada una para saber qué cara se tocó
    let mut t0 = f32::NEG_INFINITY;
    let mut t1 = f32::INFINITY;
    let mut axis0 = 0usize;
    let mut axis1 = 0usize;
    for a in 0..3 {
        let o = ray.origin.axis(a);
        let d = ray.dir.axis(a);
        let (lo, hi) = (min.axis(a), max.axis(a));
        if d.abs() < 1e-12 {
            // rayo paralelo a este par de planos: si el origen está fuera del slab
            // nunca va a entrar; si está dentro, este eje no limita nada
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        // distancias a los dos planos del slab en este eje
        let inv = 1.0 / d;
        let (mut ta, mut tb) = ((lo - o) * inv, (hi - o) * inv);
        // si el rayo va en sentido negativo, el plano "hi" se cruza primero; ordenamos
        if ta > tb {
            std::mem::swap(&mut ta, &mut tb);
        }
        if ta > t0 {
            t0 = ta;
            axis0 = a;
        }
        if tb < t1 {
            t1 = tb;
            axis1 = a;
        }
        // si la entrada ya quedó después de la salida, los intervalos no se traslapan: no hay choque
        if t0 > t1 {
            return None;
        }
    }
    // entrada si está en rango; si no, salida (rayo que nace dentro del cubo).
    let (t, axis, entering) = if t0 > t_min && t0 < t_max {
        (t0, axis0, true)
    } else if t1 > t_min && t1 < t_max {
        (t1, axis1, false)
    } else {
        return None;
    };
    // punto de impacto: origen más dirección por t
    let p = Vec3::new(
        ray.origin.x + ray.dir.x * t,
        ray.origin.y + ray.dir.y * t,
        ray.origin.z + ray.dir.z * t,
    );
    // normal saliente de la cara: al entrar se opone al rayo, al salir va con él.
    let d = ray.dir.axis(axis);
    let sign = if entering { -d.signum() } else { d.signum() };
    let normal = match axis {
        0 => Vec3::new(sign, 0.0, 0.0),
        1 => Vec3::new(0.0, sign, 0.0),
        _ => Vec3::new(0.0, 0.0, sign),
    };
    // uv por cara, en unidades de mundo (una repetición de textura por bloque),
    // orientadas para que ninguna cara quede reflejada al verla desde afuera.
    // usamos las dos coordenadas del punto que varían sobre esa cara, cambiando el
    // signo de una de ellas según hacia dónde mire la cara
    let (u, v) = match (axis, sign > 0.0) {
        (0, true) => (-p.z, p.y),
        (0, false) => (p.z, p.y),
        (1, true) => (p.x, -p.z),
        (1, false) => (p.x, p.z),
        (2, true) => (p.x, p.y),
        _ => (-p.x, p.y),
    };
    // el material se deja en cero; lo asigna `Object::intersect`
    Some(Hit { t, point: p, normal, u: u * uv_scale, v: v * uv_scale, material: 0 })
}

/// intersección rayo contra esfera de centro `c` y radio `r` dentro de (t_min, t_max).
/// resolvemos la cuadrática que sale de pedir que el punto del rayo esté a distancia r del centro.
/// devuelve el impacto más cercano válido (o el lejano si el cercano queda fuera del rango,
/// lo que pasa cuando el rayo nace dentro de la esfera), o `None`.
pub fn intersect_sphere(ray: &Ray, c: Vec3, r: f32, t_min: f32, t_max: f32) -> Option<Hit> {
    // vector del centro al origen del rayo
    let oc = ray.origin - c;
    // coeficientes de a por t al cuadrado más 2 por b por t más cc igual a cero
    // (usamos la versión con "b medio", por eso no aparecen el 2 ni el 4 de la fórmula general)
    let a = ray.dir.dot(ray.dir);
    let b = oc.dot(ray.dir);
    let cc = oc.dot(oc) - r * r;
    // discriminante reducido: si es negativo el rayo pasa de largo
    let disc = b * b - a * cc;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    // primero probamos la raíz menor (la entrada a la esfera)
    let mut t = (-b - sq) / a;
    if t <= t_min || t >= t_max {
        // si no sirve, probamos la mayor (la salida)
        t = (-b + sq) / a;
        if t <= t_min || t >= t_max {
            return None;
        }
    }
    let p = ray.origin + ray.dir * t;
    // la normal de una esfera es la dirección del centro al punto
    let n = (p - c).normalized();
    // coordenadas esféricas: u sale del ángulo alrededor del eje y (atan2), llevado a cero a uno
    let u = 0.5 + n.z.atan2(n.x) / (2.0 * std::f32::consts::PI);
    // v sale de la latitud (asin de n.y). clamp: por redondeo n.y puede pasar de 1 y asin daría nan.
    let v = 0.5 + n.y.clamp(-1.0, 1.0).asin() / std::f32::consts::PI;
    Some(Hit { t, point: p, normal: n, u, v, material: 0 })
}
