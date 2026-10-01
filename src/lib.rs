//! las noches bajo la luna eterna: raytracer en rust puro, usando solo la biblioteca estándar.
//!
//! este archivo es la raíz de la biblioteca `las_noches`. aquí solo declaramos los módulos para
//! que el ejecutable principal y las otras herramientas (como el generador de texturas) puedan usarlos.

// estructura de aceleración (árbol de cajas) para no probar cada rayo contra todos los objetos
pub mod bvh;
// cámara orbital: objetivo, inclinación, distancia y campo de visión
pub mod camera;
// figuras básicas (cubos y esferas) y sus intersecciones con rayos
pub mod geometry;
// lectura y escritura de imágenes bmp y png hechas a mano, sin librerías
pub mod image_io;
// materiales: color difuso, brillo especular, reflexión y refracción
pub mod material;
// vectores, matrices y funciones matemáticas de apoyo
pub mod math;
// el trazador de rayos, el render en paralelo y el tone mapping
pub mod renderer;
// construcción del diorama: objetos, luces, niebla y cielo
pub mod scene;
// texturas cargadas desde bmp y el skybox tipo cubemap
pub mod texture;
