//! Cámara orbital: mira a un punto fijo del diorama; el zoom cambia la DISTANCIA real
//! de la cámara (no el campo de visión). La rotación del diorama se aplica aparte
//! (renderer::View), girando el diorama sobre su eje vertical.

use crate::geometry::Ray;
use crate::math::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub target: Vec3,
    /// Inclinación sobre el horizonte, en radianes.
    pub pitch: f32,
    pub distance: f32,
    pub fov_deg: f32,
}

/// Con la distancia mínima la cámara siempre queda fuera del diorama (radio ~18.7).
pub const MIN_DISTANCE: f32 = 19.5;
pub const MAX_DISTANCE: f32 = 70.0;
pub const MIN_PITCH: f32 = 0.02;
pub const MAX_PITCH: f32 = 1.35;

impl Camera {
    pub fn clamp(&mut self) {
        self.distance = self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
        self.pitch = self.pitch.clamp(MIN_PITCH, MAX_PITCH);
    }

    pub fn position(&self) -> Vec3 {
        let (s, c) = self.pitch.sin_cos();
        self.target + Vec3::new(0.0, s, c) * self.distance
    }

    /// Base ortonormal (derecha, arriba, adelante).
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let forward = (self.target - self.position()).normalized();
        let right = forward.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
        let up = right.cross(forward);
        (right, up, forward)
    }

    /// Rayo por el punto (px, py) de la imagen (en píxeles, con subpíxel).
    #[inline]
    pub fn ray(&self, basis: &(Vec3, Vec3, Vec3), px: f32, py: f32, width: f32, height: f32) -> Ray {
        let (right, up, forward) = *basis;
        let aspect = width / height;
        let scale = (self.fov_deg.to_radians() * 0.5).tan();
        let sx = (2.0 * px / width - 1.0) * aspect * scale;
        let sy = (1.0 - 2.0 * py / height) * scale;
        Ray { origin: self.position(), dir: (forward + right * sx + up * sy).normalized() }
    }
}
