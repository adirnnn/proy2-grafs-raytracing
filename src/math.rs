//! Vectores 3D y rotaciones básicas.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const ONE: Vec3 = Vec3 { x: 1.0, y: 1.0, z: 1.0 };

    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }
    #[inline]
    pub const fn splat(v: f32) -> Self {
        Vec3 { x: v, y: v, z: v }
    }
    #[inline]
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    #[inline]
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    #[inline]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    #[inline]
    pub fn normalized(self) -> Vec3 {
        let l = self.length();
        if l > 0.0 { self * (1.0 / l) } else { self }
    }
    /// Producto componente a componente (para mezclar colores).
    #[inline]
    pub fn mul(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    #[inline]
    pub fn min(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }
    #[inline]
    pub fn max(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
    #[inline]
    pub fn axis(self, i: usize) -> f32 {
        match i {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
    #[inline]
    pub fn max_component(self) -> f32 {
        self.x.max(self.y).max(self.z)
    }
    #[inline]
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
    /// Refleja `self` (dirección incidente) respecto a la normal `n`.
    #[inline]
    pub fn reflect(self, n: Vec3) -> Vec3 {
        self - n * (2.0 * self.dot(n))
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    #[inline]
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl AddAssign for Vec3 {
    #[inline]
    fn add_assign(&mut self, o: Vec3) {
        self.x += o.x;
        self.y += o.y;
        self.z += o.z;
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    #[inline]
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Div<f32> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn div(self, s: f32) -> Vec3 {
        self * (1.0 / s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    #[inline]
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

/// Matriz de rotación 3x3 (filas).
#[derive(Clone, Copy, Debug)]
pub struct Mat3 {
    pub r: [Vec3; 3],
}

impl Mat3 {
    pub const IDENTITY: Mat3 = Mat3 {
        r: [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
    };

    pub fn rot_x(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3 { r: [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, c, -s), Vec3::new(0.0, s, c)] }
    }
    pub fn rot_y(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3 { r: [Vec3::new(c, 0.0, s), Vec3::new(0.0, 1.0, 0.0), Vec3::new(-s, 0.0, c)] }
    }
    pub fn rot_z(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3 { r: [Vec3::new(c, -s, 0.0), Vec3::new(s, c, 0.0), Vec3::new(0.0, 0.0, 1.0)] }
    }
    #[inline]
    pub fn apply(&self, v: Vec3) -> Vec3 {
        Vec3::new(self.r[0].dot(v), self.r[1].dot(v), self.r[2].dot(v))
    }
    /// Transpuesta = inversa para rotaciones puras.
    pub fn transpose(&self) -> Mat3 {
        let [a, b, c] = self.r;
        Mat3 {
            r: [Vec3::new(a.x, b.x, c.x), Vec3::new(a.y, b.y, c.y), Vec3::new(a.z, b.z, c.z)],
        }
    }
    pub fn mul(&self, o: &Mat3) -> Mat3 {
        let t = o.transpose();
        let mut r = [Vec3::ZERO; 3];
        for i in 0..3 {
            r[i] = Vec3::new(self.r[i].dot(t.r[0]), self.r[i].dot(t.r[1]), self.r[i].dot(t.r[2]));
        }
        Mat3 { r }
    }
}

/// Generador pseudoaleatorio determinista (PCG-hash). Mismo seed => misma imagen.
#[derive(Clone, Copy)]
pub struct Rng(pub u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(hash_u32(seed ^ 0x9E37_79B9))
    }
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        let w = ((self.0 >> ((self.0 >> 28) + 4)) ^ self.0).wrapping_mul(277_803_737);
        ((w >> 22) ^ w) as f32 / u32::MAX as f32
    }
}

#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
