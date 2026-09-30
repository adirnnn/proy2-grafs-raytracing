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
pub const STEEL: u16 = 4;
pub const FAKE_SKY: u16 = 5;
pub const BONE: u16 = 6;
pub const BASALT: u16 = 7;
pub const HILT: u16 = 8;

pub const MATERIALS: [Material; 9] = [
    // 1. Arena blanca de Hueco Mundo: mate, casi sin brillo.
    Material { albedo: 0.85, specular: 0.04, shininess: 8.0, ..Material::base("Arena de Hueco Mundo", "sand") },
    // 2. Piedra/yeso de Las Noches: satinado, un poco de reflejo.
    Material {
        albedo: 0.82,
        specular: 0.25,
        shininess: 40.0,
        reflectivity: 0.06,
        uv_scale: 0.5,
        ..Material::base("Piedra de Las Noches", "stone")
    },
    // 3. Cuarzo de los árboles de cristal: transparente y refractivo.
    Material {
        albedo: 0.10,
        specular: 0.9,
        shininess: 220.0,
        transparency: 0.82,
        reflectivity: 0.06,
        ior: 1.54,
        transmit_tint: Vec3::new(0.86, 0.96, 1.0),
        ..Material::base("Cuarzo", "quartz")
    },
    // 4. Obsidiana pulida de la plaza: espejo oscuro.
    Material {
        albedo: 0.20,
        specular: 0.9,
        shininess: 300.0,
        reflectivity: 0.55,
        ..Material::base("Obsidiana", "obsidian")
    },
    // 5. Acero de la espada: metálico, muy reflectivo.
    Material {
        albedo: 0.18,
        specular: 1.0,
        shininess: 500.0,
        reflectivity: 0.78,
        ..Material::base("Acero de Zangetsu", "steel")
    },
    // Extra: interior de la cúpula ("cielo falso" de Las Noches), emisivo.
    Material { albedo: 0.35, emission: 1.1, ..Material::base("Cielo falso", "fake_sky") },
    // Extra: hueso de la máscara Hollow.
    Material {
        albedo: 0.85,
        specular: 0.4,
        shininess: 60.0,
        reflectivity: 0.04,
        ..Material::base("Hueso Hollow", "bone")
    },
    // Extra: basalto del zócalo del diorama.
    Material { albedo: 0.7, specular: 0.15, shininess: 20.0, ..Material::base("Basalto", "basalt") },
    // Extra: vendaje del mango de la espada.
    Material { albedo: 0.8, specular: 0.05, shininess: 10.0, ..Material::base("Vendaje", "hilt") },
];
