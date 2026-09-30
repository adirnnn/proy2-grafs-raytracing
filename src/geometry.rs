//! Primitivas: cubos (cajas alineadas, opcionalmente rotadas) y esferas.
//! Todo vive en el "espacio del diorama"; el renderer transforma los rayos a este
//! espacio para rotar el diorama sin reconstruir la BVH.

use crate::math::{Mat3, Vec3};

#[derive(Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
}

/// Resultado de una intersección.
#[derive(Clone, Copy)]
pub struct Hit {
    pub t: f32,
    pub point: Vec3,
    /// Normal geométrica apuntando hacia AFUERA del objeto.
    pub normal: Vec3,
    pub u: f32,
    pub v: f32,
    pub material: u16,
}

#[derive(Clone, Copy)]
pub enum Shape {
    /// Caja alineada a los ejes (en el espacio local si `rot` existe).
    Cube { min: Vec3, max: Vec3 },
    Sphere { center: Vec3, radius: f32 },
}

#[derive(Clone, Copy)]
pub struct Object {
    pub shape: Shape,
    pub material: u16,
    /// Material alterno para caras cuya normal apunta hacia `alt_center`
    /// (se usa para la cara interior de la cúpula: afuera yeso, adentro "cielo falso").
    pub alt: Option<(u16, Vec3)>,
    /// Rotación opcional alrededor de `pivot` (para la espada inclinada, cristales, etc.).
    pub rot: Option<(Mat3, Vec3)>,
    /// Escala de UV: cuántas repeticiones de textura por unidad de mundo.
    pub uv_scale: f32,
}

impl Object {
    pub fn cube(min: Vec3, max: Vec3, material: u16) -> Object {
        Object { shape: Shape::Cube { min, max }, material, alt: None, rot: None, uv_scale: 1.0 }
    }
    pub fn sphere(center: Vec3, radius: f32, material: u16) -> Object {
        Object { shape: Shape::Sphere { center, radius }, material, alt: None, rot: None, uv_scale: 1.0 }
    }
    pub fn rotated(mut self, m: Mat3, pivot: Vec3) -> Object {
        self.rot = Some((m, pivot));
        self
    }
    pub fn with_uv_scale(mut self, s: f32) -> Object {
        self.uv_scale = s;
        self
    }

    /// Caja envolvente en espacio del diorama (para la BVH).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (lo, hi) = match self.shape {
            Shape::Cube { min, max } => (min, max),
            Shape::Sphere { center, radius } => (center - Vec3::splat(radius), center + Vec3::splat(radius)),
        };
        match self.rot {
            None => (lo, hi),
            Some((m, p)) => {
                let mut bmin = Vec3::splat(f32::INFINITY);
                let mut bmax = Vec3::splat(f32::NEG_INFINITY);
                for i in 0..8 {
                    let c = Vec3::new(
                        if i & 1 == 0 { lo.x } else { hi.x },
                        if i & 2 == 0 { lo.y } else { hi.y },
                        if i & 4 == 0 { lo.z } else { hi.z },
                    );
                    let w = m.apply(c - p) + p;
                    bmin = bmin.min(w);
                    bmax = bmax.max(w);
                }
                (bmin, bmax)
            }
        }
    }

    pub fn intersect(&self, ray: &Ray, t_min: f32, t_max: f32) -> Option<Hit> {
        // Si el objeto está rotado, llevamos el rayo a su espacio local con la inversa
        // (transpuesta) de la rotación, y regresamos punto y normal con la rotación.
        let local = match self.rot {
            None => *ray,
            Some((m, p)) => {
                let mt = m.transpose();
                Ray { origin: mt.apply(ray.origin - p) + p, dir: mt.apply(ray.dir) }
            }
        };
        let mut hit = match self.shape {
            Shape::Cube { min, max } => intersect_cube(&local, min, max, t_min, t_max, self.uv_scale)?,
            Shape::Sphere { center, radius } => intersect_sphere(&local, center, radius, t_min, t_max)?,
        };
        if let Some((m, p)) = self.rot {
            // Rotación pura: la normal se transforma con la misma matriz (inversa-transpuesta = m).
            hit.normal = m.apply(hit.normal);
            hit.point = m.apply(hit.point - p) + p;
        }
        hit.material = self.material;
        if let Some((alt, c)) = self.alt {
            if hit.normal.dot(c - hit.point) > 0.0 {
                hit.material = alt;
            }
        }
        Some(hit)
    }
}

/// Intersección rayo-caja por "slabs". Devuelve también el punto de SALIDA si el
/// origen está dentro (necesario para refracción al salir de un cubo de cristal).
pub fn intersect_cube(ray: &Ray, min: Vec3, max: Vec3, t_min: f32, t_max: f32, uv_scale: f32) -> Option<Hit> {
    let mut t0 = f32::NEG_INFINITY;
    let mut t1 = f32::INFINITY;
    let mut axis0 = 0usize;
    let mut axis1 = 0usize;
    for a in 0..3 {
        let o = ray.origin.axis(a);
        let d = ray.dir.axis(a);
        let (lo, hi) = (min.axis(a), max.axis(a));
        if d.abs() < 1e-12 {
            if o < lo || o > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / d;
        let (mut ta, mut tb) = ((lo - o) * inv, (hi - o) * inv);
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
        if t0 > t1 {
            return None;
        }
    }
    // Entrada si está en rango; si no, salida (rayo que nace dentro del cubo).
    let (t, axis, entering) = if t0 > t_min && t0 < t_max {
        (t0, axis0, true)
    } else if t1 > t_min && t1 < t_max {
        (t1, axis1, false)
    } else {
        return None;
    };
    let p = Vec3::new(
        ray.origin.x + ray.dir.x * t,
        ray.origin.y + ray.dir.y * t,
        ray.origin.z + ray.dir.z * t,
    );
    // Normal saliente de la cara: al entrar se opone al rayo, al salir va con él.
    let d = ray.dir.axis(axis);
    let sign = if entering { -d.signum() } else { d.signum() };
    let normal = match axis {
        0 => Vec3::new(sign, 0.0, 0.0),
        1 => Vec3::new(0.0, sign, 0.0),
        _ => Vec3::new(0.0, 0.0, sign),
    };
    // UV por cara, en unidades de mundo (una repetición de textura por bloque),
    // orientadas para que ninguna cara quede reflejada al verla desde afuera.
    let (u, v) = match (axis, sign > 0.0) {
        (0, true) => (-p.z, p.y),
        (0, false) => (p.z, p.y),
        (1, true) => (p.x, -p.z),
        (1, false) => (p.x, p.z),
        (2, true) => (p.x, p.y),
        _ => (-p.x, p.y),
    };
    Some(Hit { t, point: p, normal, u: u * uv_scale, v: v * uv_scale, material: 0 })
}

pub fn intersect_sphere(ray: &Ray, c: Vec3, r: f32, t_min: f32, t_max: f32) -> Option<Hit> {
    let oc = ray.origin - c;
    let a = ray.dir.dot(ray.dir);
    let b = oc.dot(ray.dir);
    let cc = oc.dot(oc) - r * r;
    let disc = b * b - a * cc;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let mut t = (-b - sq) / a;
    if t <= t_min || t >= t_max {
        t = (-b + sq) / a;
        if t <= t_min || t >= t_max {
            return None;
        }
    }
    let p = ray.origin + ray.dir * t;
    let n = (p - c) / r;
    let u = 0.5 + n.z.atan2(n.x) / (2.0 * std::f32::consts::PI);
    let v = 0.5 + n.y.asin() / std::f32::consts::PI;
    Some(Hit { t, point: p, normal: n, u, v, material: 0 })
}
