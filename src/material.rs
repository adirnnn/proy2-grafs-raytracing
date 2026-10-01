//! Materiales del diorama.
//!
//! Interpretación de los parámetros (ver renderer::shade):
//! - `albedo`: fracción de luz difusa que devuelve la superficie; se multiplica por el
//!   color de su textura (textura en espacio lineal).
//! - `specular`: intensidad del brillo especular de Phong; `shininess` es su exponente
//!   (más alto = brillo más pequeño y nítido).
//! - `transparency`: fracción de luz que atraviesa el material (rayo refractado).
//! - `reflectivity`: fracción de luz reflejada como espejo (rayo reflejado).
//! - `ior`: índice de refracción, usado solo si `transparency > 0`.
//! - `emission`: brillo propio (la superficie se ve encendida, pero NO ilumina a otros
//!   objetos; la luz que sale de la cúpula la aporta una luz puntual aparte).
//!
//! Peso de la parte "local" (difusa) = 1 - reflectivity - transparency. Con Fresnel
//! (Schlick), parte de la transparencia se vuelve reflexión en ángulos rasantes, así que
//! la suma difusa + reflejada + transmitida nunca pasa de 1.

use crate::math::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub name: &'static str,
    /// Archivo en assets/textures/ (sin extensión).
    pub texture: &'static str,
    pub albedo: f32,
    pub specular: f32,
    pub shininess: f32,
    pub transparency: f32,
    pub reflectivity: f32,
    pub ior: f32,
    pub emission: f32,
    /// Tinte que se aplica a la luz que atraviesa el material (color del cristal).
    pub transmit_tint: Vec3,
    /// Repeticiones de la textura por bloque (0.5 = una textura cada 2 bloques).
    pub uv_scale: f32,
}

impl Material {
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

// Índices de material (orden de MATERIALS).
pub const SAND: u16 = 0;
pub const STONE: u16 = 1;
pub const QUARTZ: u16 = 2;
pub const OBSIDIAN: u16 = 3;
pub const MARBLE: u16 = 4;
pub const FAKE_SKY: u16 = 5;
pub const BASALT: u16 = 6;

pub const MATERIALS: [Material; 7] = [
    // 1. Arena de Hueco Mundo: blanca grisácea, mate, sin reflejo.
    Material { albedo: 0.66, specular: 0.04, shininess: 8.0, ..Material::base("Arena de Hueco Mundo", "sand") },
    // 2. Concreto blanco de Las Noches: satinado, reflejo mínimo.
    Material {
        albedo: 0.72,
        specular: 0.18,
        shininess: 30.0,
        reflectivity: 0.04,
        uv_scale: 0.5,
        ..Material::base("Concreto de Las Noches", "stone")
    },
    // 3. Cuarzo de los árboles muertos: translúcido y refractivo.
    Material {
        albedo: 0.22,
        specular: 0.8,
        shininess: 180.0,
        transparency: 0.70,
        reflectivity: 0.06,
        ior: 1.54,
        transmit_tint: Vec3::new(0.94, 0.97, 1.0),
        ..Material::base("Cuarzo", "quartz")
    },
    // 4. Obsidiana pulida de los monolitos: espejo oscuro.
    Material {
        albedo: 0.18,
        specular: 0.9,
        shininess: 300.0,
        reflectivity: 0.60,
        ..Material::base("Obsidiana", "obsidian")
    },
    // 5. Mármol pulido del salón del trono: blanco con vetas, refleja el cielo falso.
    Material {
        albedo: 0.62,
        specular: 0.7,
        shininess: 160.0,
        reflectivity: 0.35,
        uv_scale: 0.5,
        ..Material::base("Mármol pulido", "marble")
    },
    // Extra: interior de la cúpula ("cielo falso" de Las Noches), emisivo.
    Material { albedo: 0.35, emission: 1.0, uv_scale: 0.125, ..Material::base("Cielo falso", "fake_sky") },
    // Extra: basalto del zócalo del diorama.
    Material { albedo: 0.6, specular: 0.15, shininess: 20.0, ..Material::base("Basalto", "basalt") },
];
