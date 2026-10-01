//! cámara orbital: siempre mira a un punto fijo del diorama y el zoom cambia la
//! distancia real de la cámara (no el campo de visión). la rotación del diorama se
//! aplica aparte (`renderer::View`), girando el diorama sobre su eje vertical, así que
//! la cámara solo necesita inclinación y distancia.

use crate::geometry::Ray;
use crate::math::Vec3;

/// estado de la cámara orbital.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// punto al que mira la cámara.
    pub target: Vec3,
    /// inclinación sobre el horizonte, en radianes (positiva es mirar desde arriba).
    pub pitch: f32,
    /// distancia entre la cámara y `target`.
    pub distance: f32,
    /// campo de visión vertical, en grados.
    pub fov_deg: f32,
}

/// distancia mínima. con ella la cámara queda fuera de todo lo que sobresale del suelo
/// (un radio aproximado de 41 desde el pivote; una prueba lo verifica).
pub const MIN_DISTANCE: f32 = 46.0;
/// distancia máxima de la cámara.
pub const MAX_DISTANCE: f32 = 140.0;
/// inclinación mínima en radianes (un poco por debajo del horizonte, mirando hacia arriba).
pub const MIN_PITCH: f32 = -0.30;
/// inclinación máxima en radianes; se queda por debajo de 90 grados para que la base
/// de la cámara nunca se degenere al mirar justo hacia abajo.
pub const MAX_PITCH: f32 = 1.35;
/// altura mínima de la cámara: a la altura de los ojos de una persona (1 unidad
/// equivale aproximadamente a 4 m).
pub const MIN_HEIGHT: f32 = 1.4;

impl Camera {
    /// mantiene la distancia y la inclinación dentro de los límites permitidos.
    /// se llama cada vez que el usuario mueve la cámara.
    pub fn clamp(&mut self) {
        self.distance = self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
        // la inclinación mínima depende de la distancia, para nunca bajar del suelo.
        // la altura de la cámara es target.y más distance por sin(pitch); pedimos que sea al
        // menos `MIN_HEIGHT`, despejamos sin(pitch) y sacamos el arcoseno. el clamp a
        // [menos 1, 1] evita un nan en asin, y al final respetamos también `MIN_PITCH`.
        let min_pitch = ((MIN_HEIGHT - self.target.y) / self.distance).clamp(-1.0, 1.0).asin().max(MIN_PITCH);
        self.pitch = self.pitch.clamp(min_pitch, MAX_PITCH);
    }

    /// posición de la cámara en el mundo. está sobre una circunferencia vertical
    /// alrededor de `target`, del lado de z positivo, elevada según `pitch`.
    pub fn position(&self) -> Vec3 {
        let (s, c) = self.pitch.sin_cos();
        // (0, sin, cos) es un vector unitario: el seno da la altura y el coseno el alejamiento en z.
        self.target + Vec3::new(0.0, s, c) * self.distance
    }

    /// base ortonormal de la cámara como (derecha, arriba, adelante).
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        // adelante: de la cámara hacia el objetivo.
        let forward = (self.target - self.position()).normalized();
        // derecha: perpendicular a adelante y al eje y del mundo (producto cruz).
        let right = forward.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
        // arriba: perpendicular a las dos anteriores; ya sale unitario porque son ortonormales.
        let up = right.cross(forward);
        (right, up, forward)
    }

    /// genera el rayo primario que pasa por el punto (px, py) de la imagen, en píxeles
    /// y con posición subpíxel. recibe la base ya calculada (para no recalcularla en
    /// cada píxel) y el tamaño de la imagen; devuelve un rayo con dirección normalizada.
    #[inline]
    pub fn ray(&self, basis: &(Vec3, Vec3, Vec3), px: f32, py: f32, width: f32, height: f32) -> Ray {
        let (right, up, forward) = *basis;
        let aspect = width / height;
        // medio alto del plano de imagen a distancia 1: tangente de la mitad del fov.
        let scale = (self.fov_deg.to_radians() * 0.5).tan();
        // px va de 0 a width y lo pasamos al rango de menos 1 a 1; en x multiplicamos por
        // el aspecto para que los píxeles salgan cuadrados.
        let sx = (2.0 * px / width - 1.0) * aspect * scale;
        // py crece hacia abajo en la imagen, por eso usamos 1 menos el valor.
        let sy = (1.0 - 2.0 * py / height) * scale;
        // el rayo sale de la cámara y apunta hacia ese punto del plano de imagen.
        Ray { origin: self.position(), dir: (forward + right * sx + up * sy).normalized() }
    }
}
