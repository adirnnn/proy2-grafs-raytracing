//! materiales del diorama.
//!
//! aquí definimos la estructura `Material` y la tabla `MATERIALS` con los nueve
//! materiales de la escena. cada objeto guarda solo un índice (`u16`) a esta tabla.
//!
//! cómo interpretamos los parámetros (ver `renderer::shade`):
//! * `albedo`: fracción de luz difusa que devuelve la superficie; se multiplica por el
//!   color de su textura (la textura ya está en espacio lineal).
//! * `specular`: intensidad del brillo especular de phong; `shininess` es su exponente
//!   (mientras más alto, el brillo es más pequeño y nítido).
//! * `transparency`: fracción de luz que atraviesa el material (rayo refractado).
//! * `reflectivity`: fracción de luz reflejada como espejo (rayo reflejado).
//! * `ior`: índice de refracción, solo se usa si `transparency` es mayor que 0.
//! * `emission`: brillo propio. la superficie se ve encendida pero no ilumina a otros
//!   objetos; la luz que sale de la cúpula la pone una luz puntual aparte.
//!
//! el peso de la parte local (difusa) es uno menos `reflectivity` menos `transparency`.
//! con fresnel (aproximación de schlick) parte de la transparencia se convierte en
//! reflexión en ángulos rasantes, así que la suma difusa más reflejada más transmitida
//! nunca pasa de 1.

use crate::math::Vec3;

/// propiedades ópticas de una superficie. es `Copy` porque solo tiene números y textos
/// estáticos, así que podemos pasarla por valor sin costo.
#[derive(Clone, Copy, Debug)]
pub struct Material {
    /// nombre legible del material, para identificarlo al leer la tabla.
    pub name: &'static str,
    /// archivo en assets/textures/ (sin la extensión .bmp).
    pub texture: &'static str,
    /// fracción de luz difusa que refleja la superficie (de 0 a 1).
    pub albedo: f32,
    /// intensidad del brillo especular de phong.
    pub specular: f32,
    /// exponente de phong: controla qué tan concentrado es el brillo.
    pub shininess: f32,
    /// fracción de luz que pasa a través del material.
    pub transparency: f32,
    /// fracción de luz que se refleja como en un espejo.
    pub reflectivity: f32,
    /// índice de refracción (1.0 es el aire, 1.54 el cuarzo).
    pub ior: f32,
    /// brillo propio del material, sumado sin depender de las luces.
    pub emission: f32,
    /// tinte que se aplica a la luz que atraviesa el material (color del cristal).
    pub transmit_tint: Vec3,
    /// repeticiones de la textura por bloque (0.5 significa una textura cada 2 bloques).
    pub uv_scale: f32,
}

impl Material {
    /// material base con valores neutros: difuso mate (albedo 0.8), sin brillo, sin
    /// transparencia, sin reflejo y sin emisión. recibe el nombre y la textura y
    /// devuelve el material; en la tabla solo cambiamos los campos que nos interesan
    /// con la sintaxis `..Material::base(...)`. es `const fn` para poder usarlo en una constante.
    const fn base(name: &'static str, texture: &'static str) -> Material {
        Material {
            name,
            texture,
            albedo: 0.8,
            specular: 0.0,
            shininess: 1.0,
            transparency: 0.0,
            reflectivity: 0.0,
            ior: 1.0,
            emission: 0.0,
            transmit_tint: Vec3::ONE,
            uv_scale: 1.0,
        }
    }
}

// índices de material: tienen que coincidir con el orden de `MATERIALS`.
/// índice de la arena del desierto.
pub const SAND: u16 = 0;
/// índice del concreto blanco de la fortaleza y las torres.
pub const STONE: u16 = 1;
/// índice del cuarzo de los árboles muertos.
pub const QUARTZ: u16 = 2;
/// índice de la obsidiana pulida (calzada, monolitos y lomo de la espada).
pub const OBSIDIAN: u16 = 3;
/// índice del mármol pulido del piso interior.
pub const MARBLE: u16 = 4;
/// índice del cielo falso que se ve dentro de la cúpula.
pub const FAKE_SKY: u16 = 5;
/// índice del acero de la hoja de zangetsu.
pub const STEEL: u16 = 6;
/// índice de las ventanas y vanos encendidos.
pub const LIGHT: u16 = 7;
/// índice del vendaje del mango de zangetsu.
pub const HILT: u16 = 8;

/// tabla con todos los materiales de la escena. los cinco primeros son los materiales
/// principales del proyecto y los demás son extras para detalles.
pub const MATERIALS: [Material; 9] = [
    // 1. arena de hueco mundo: blanca grisácea, mate y sin reflejo. la textura se repite
    // 3 veces por bloque para que el grano se vea fino.
    Material { albedo: 0.66, specular: 0.04, shininess: 8.0, uv_scale: 3.0, ..Material::base("Arena de Hueco Mundo", "sand") },
    // 2. concreto blanco de las noches: satinado, con un reflejo mínimo (4 por ciento).
    Material {
        albedo: 0.72,
        specular: 0.18,
        shininess: 30.0,
        reflectivity: 0.04,
        uv_scale: 0.5,
        ..Material::base("Concreto de Las Noches", "stone")
    },
    // 3. cuarzo de los árboles muertos: translúcido y refractivo. deja pasar el 70 por
    // ciento de la luz, con el índice de refracción del cuarzo y un tinte apenas azulado.
    Material {
        albedo: 0.32,
        specular: 0.8,
        shininess: 180.0,
        transparency: 0.70,
        reflectivity: 0.06,
        ior: 1.54,
        transmit_tint: Vec3::new(0.94, 0.97, 1.0),
        ..Material::base("Cuarzo", "quartz")
    },
    // 4. obsidiana pulida (calzada y monolitos): espejo oscuro, poco difuso y con un
    // brillo especular muy nítido.
    Material {
        albedo: 0.15,
        specular: 0.9,
        shininess: 300.0,
        reflectivity: 0.65,
        ..Material::base("Obsidiana", "obsidian")
    },
    // 5. mármol pulido del salón del trono: blanco con vetas, refleja el cielo falso.
    Material {
        albedo: 0.62,
        specular: 0.7,
        shininess: 160.0,
        reflectivity: 0.35,
        uv_scale: 0.5,
        ..Material::base("Mármol pulido", "marble")
    },
    // extra: interior de la cúpula ("cielo falso" de las noches), emisivo. la textura
    // se estira mucho (una cada 8 bloques) para que las nubes se vean grandes.
    Material { albedo: 0.35, emission: 1.0, uv_scale: 0.125, ..Material::base("Cielo falso", "fake_sky") },
    // extra: acero de zangetsu, metálico y muy reflectivo.
    Material {
        albedo: 0.22,
        specular: 1.0,
        shininess: 500.0,
        reflectivity: 0.80,
        ..Material::base("Acero de Zangetsu", "steel")
    },
    // extra: ventanas encendidas (emisivas; no iluminan a otros objetos).
    Material { albedo: 0.2, emission: 2.2, ..Material::base("Ventanas encendidas", "light") },
    // extra: vendaje del mango de zangetsu, tela mate.
    Material { albedo: 0.8, specular: 0.05, shininess: 10.0, ..Material::base("Vendaje", "hilt") },
];
