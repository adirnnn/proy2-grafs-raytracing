//! módulo de matemática básica del raytracer.
//!
//! aquí definimos todo lo que el resto del programa necesita para hacer cuentas:
//! el vector de tres componentes `Vec3` (lo usamos para puntos, direcciones y
//! también para colores rgb), la matriz de rotación `Mat3` (para girar el diorama
//! y los objetos inclinados), un generador pseudoaleatorio `Rng` (para el jitter
//! del antialiasing y las sombras suaves) y un par de funciones auxiliares
//! (`hash_u32` y `smoothstep`). no depende de ningún otro módulo del proyecto,
//! así que es la base sobre la que se construyen geometry, bvh y renderer.

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// vector de tres flotantes. lo usamos para posiciones, direcciones, normales y
/// colores (en ese caso x, y, z son r, g, b en espacio lineal).
/// es `Copy` para poder pasarlo por valor sin preocuparnos de préstamos.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    /// componente x (o rojo si es un color).
    pub x: f32,
    /// componente y (o verde si es un color).
    pub y: f32,
    /// componente z (o azul si es un color).
    pub z: f32,
}

impl Vec3 {
    /// vector nulo, útil como acumulador inicial o color negro.
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    /// vector de unos, útil como color blanco o factor neutro al multiplicar.
    pub const ONE: Vec3 = Vec3 { x: 1.0, y: 1.0, z: 1.0 };

    /// construye un vector a partir de sus tres componentes.
    /// es `const` para poder usarlo al definir constantes.
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }
    /// construye un vector con las tres componentes iguales a `v`.
    #[inline]
    pub const fn splat(v: f32) -> Self {
        Vec3 { x: v, y: v, z: v }
    }
    /// producto punto: suma de los productos componente a componente.
    /// si ambos vectores son unitarios, el resultado es el coseno del ángulo entre ellos,
    /// por eso lo usamos tanto en iluminación (lambert) como en reflexión y refracción.
    #[inline]
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    /// producto cruz: devuelve un vector perpendicular a `self` y a `o`
    /// (regla de la mano derecha). lo usamos para armar la base de la cámara.
    #[inline]
    pub fn cross(self, o: Vec3) -> Vec3 {
        // fórmula clásica del determinante con los vectores unitarios i, j, k
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    /// longitud (norma euclidiana) del vector: raíz cuadrada de su producto punto consigo mismo.
    #[inline]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    /// devuelve el vector con la misma dirección pero longitud uno.
    /// si la longitud es cero lo devolvemos tal cual para no dividir entre cero.
    #[inline]
    pub fn normalized(self) -> Vec3 {
        let l = self.length();
        // multiplicamos por el inverso en vez de dividir tres veces
        if l > 0.0 { self * (1.0 / l) } else { self }
    }
    /// producto componente a componente (para mezclar colores, por ejemplo luz por textura).
    #[inline]
    pub fn mul(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
    /// mínimo por componente. lo usamos para ir creciendo cajas envolventes.
    #[inline]
    pub fn min(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.min(o.x), self.y.min(o.y), self.z.min(o.z))
    }
    /// máximo por componente, la contraparte de `min`.
    #[inline]
    pub fn max(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x.max(o.x), self.y.max(o.y), self.z.max(o.z))
    }
    /// accede a una componente por índice: 0 es x, 1 es y, cualquier otro es z.
    /// nos sirve para escribir ciclos sobre los tres ejes (slabs, bvh).
    #[inline]
    pub fn axis(self, i: usize) -> f32 {
        match i {
            0 => self.x,
            1 => self.y,
            _ => self.z,
        }
    }
    /// la componente más grande. en colores la usamos para saber si una luz o una
    /// atenuación ya es tan pequeña que no vale la pena seguir calculando.
    #[inline]
    pub fn max_component(self) -> f32 {
        self.x.max(self.y).max(self.z)
    }
    /// interpolación lineal: con `t` igual a cero da `self`, con `t` igual a uno da `o`.
    #[inline]
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        // equivalente a self por (1 menos t) más o por t, pero con una multiplicación menos
        self + (o - self) * t
    }
    /// refleja `self` (dirección incidente) respecto a la normal `n`.
    /// fórmula: r igual a d menos 2 por (d punto n) por n, con `n` unitaria.
    #[inline]
    pub fn reflect(self, n: Vec3) -> Vec3 {
        // quitamos dos veces la parte de la dirección que va a lo largo de la normal
        self - n * (2.0 * self.dot(n))
    }
}

// sobrecarga de operadores para poder escribir las fórmulas de forma natural (a + b, a * s, etc.)

/// suma de vectores componente a componente.
impl Add for Vec3 {
    type Output = Vec3;
    #[inline]
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
/// suma en el lugar (`a += b`), muy usada para acumular color.
impl AddAssign for Vec3 {
    #[inline]
    fn add_assign(&mut self, o: Vec3) {
        self.x += o.x;
        self.y += o.y;
        self.z += o.z;
    }
}
/// resta de vectores componente a componente.
impl Sub for Vec3 {
    type Output = Vec3;
    #[inline]
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
/// multiplicación por un escalar (escala el vector).
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
/// división por un escalar, implementada como multiplicación por el inverso.
impl Div<f32> for Vec3 {
    type Output = Vec3;
    #[inline]
    fn div(self, s: f32) -> Vec3 {
        self * (1.0 / s)
    }
}
/// negación: invierte el sentido del vector (por ejemplo para voltear una normal).
impl Neg for Vec3 {
    type Output = Vec3;
    #[inline]
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

/// matriz de rotación 3x3 guardada por filas.
/// aplicar la matriz a un vector es hacer el producto punto de cada fila con el vector.
#[derive(Clone, Copy, Debug)]
pub struct Mat3 {
    /// las tres filas de la matriz.
    pub r: [Vec3; 3],
}

impl Mat3 {
    /// matriz identidad: no rota nada.
    pub const IDENTITY: Mat3 = Mat3 {
        r: [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ],
    };

    /// rotación de `a` radianes alrededor del eje x (deja x fijo y gira el plano yz).
    pub fn rot_x(a: f32) -> Mat3 {
        // sin_cos calcula seno y coseno de una sola vez
        let (s, c) = a.sin_cos();
        Mat3 { r: [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, c, -s), Vec3::new(0.0, s, c)] }
    }
    /// rotación de `a` radianes alrededor del eje y (deja y fijo y gira el plano xz).
    /// es la que usamos para girar el diorama completo (el yaw de la vista).
    pub fn rot_y(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        // ojo: en la rotación en y el signo del seno queda en la fila de z, no en la de x
        Mat3 { r: [Vec3::new(c, 0.0, s), Vec3::new(0.0, 1.0, 0.0), Vec3::new(-s, 0.0, c)] }
    }
    /// rotación de `a` radianes alrededor del eje z (deja z fijo y gira el plano xy).
    pub fn rot_z(a: f32) -> Mat3 {
        let (s, c) = a.sin_cos();
        Mat3 { r: [Vec3::new(c, -s, 0.0), Vec3::new(s, c, 0.0), Vec3::new(0.0, 0.0, 1.0)] }
    }
    /// aplica la matriz al vector `v` (producto matriz por vector columna).
    /// cada componente del resultado es el producto punto de una fila con `v`.
    #[inline]
    pub fn apply(&self, v: Vec3) -> Vec3 {
        Vec3::new(self.r[0].dot(v), self.r[1].dot(v), self.r[2].dot(v))
    }
    /// devuelve la transpuesta (las filas pasan a ser columnas).
    /// como las rotaciones puras son ortonormales, la transpuesta es igual a la inversa,
    /// así deshacemos una rotación sin tener que invertir una matriz de verdad.
    pub fn transpose(&self) -> Mat3 {
        let [a, b, c] = self.r;
        // la nueva fila i se arma con la componente i de cada fila original
        Mat3 {
            r: [Vec3::new(a.x, b.x, c.x), Vec3::new(a.y, b.y, c.y), Vec3::new(a.z, b.z, c.z)],
        }
    }
    /// producto de matrices `self` por `o`. el resultado aplica primero `o` y luego `self`.
    pub fn mul(&self, o: &Mat3) -> Mat3 {
        // transponemos `o` para que sus columnas queden como filas y así
        // cada entrada del producto sea un simple producto punto entre filas
        let t = o.transpose();
        let mut r = [Vec3::ZERO; 3];
        for i in 0..3 {
            // entrada (i, j) es la fila i de self punto la columna j de o
            r[i] = Vec3::new(self.r[i].dot(t.r[0]), self.r[i].dot(t.r[1]), self.r[i].dot(t.r[2]));
        }
        Mat3 { r }
    }
}

/// generador pseudoaleatorio determinista basado en pcg (con un hash a la salida).
/// el estado es un solo `u32`; con la misma semilla siempre sale la misma secuencia,
/// y por eso el render da exactamente la misma imagen cada vez que lo corremos.
#[derive(Clone, Copy)]
pub struct Rng(pub u32);

impl Rng {
    /// crea un generador a partir de una semilla. primero mezclamos la semilla con
    /// una constante (la razón áurea en hexadecimal) y luego con `hash_u32`, para que
    /// semillas parecidas (píxeles vecinos) no den secuencias parecidas.
    pub fn new(seed: u32) -> Rng {
        Rng(hash_u32(seed ^ 0x9E37_79B9))
    }
    /// avanza el estado y devuelve un flotante en el rango de cero a uno.
    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        // paso lineal congruencial: estado por multiplicador más incremento (con desborde permitido)
        self.0 = self.0.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
        // permutación de salida tipo pcg: un corrimiento que depende de los bits altos
        // del propio estado, luego xor y otra multiplicación para revolver bien los bits
        let w = ((self.0 >> ((self.0 >> 28) + 4)) ^ self.0).wrapping_mul(277_803_737);
        // último xor con corrimiento y dividimos entre el máximo de u32 para quedar entre cero y uno
        ((w >> 22) ^ w) as f32 / u32::MAX as f32
    }
}

/// función hash de enteros de 32 bits (estilo "lowbias32").
/// alterna xor con corrimientos y multiplicaciones por constantes impares para que
/// cambiar un solo bit de la entrada cambie aproximadamente la mitad de los bits de salida.
#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    // mezclamos los bits altos sobre los bajos
    x ^= x >> 16;
    // multiplicación con desborde: esparce los bits hacia arriba
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    // último plegado para que los bits altos también afecten los bajos
    x ^= x >> 16;
    x
}

/// interpolación suave tipo hermite entre los bordes `e0` y `e1`.
/// devuelve cero si `x` está antes de `e0`, uno si está después de `e1`
/// y una curva en forma de s entre ambos (sin cambios bruscos de pendiente).
#[inline]
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    // normalizamos x al rango de cero a uno y recortamos
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    // polinomio 3t al cuadrado menos 2t al cubo, escrito como t por t por (3 menos 2t)
    t * t * (3.0 - 2.0 * t)
}
