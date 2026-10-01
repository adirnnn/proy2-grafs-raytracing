//! escenas de diagnostico para revisar a ojo que la optica del raytracer este bien:
//! refraccion, reflexion, reflexion interna total, la orientacion uv de las caras y que
//! el skybox aparezca en los rayos secundarios (reflejados y refractados).
//!
//! se activa corriendo el programa principal en modo release con la opcion `diag`
//! seguida de la carpeta donde se guardan las imagenes.
//!
//! la escena tiene un piso y una pared con tablero de ajedrez, y delante cinco objetos
//! de prueba (a, b, c, d, e) que se explican uno por uno abajo.

use crate::render_image;
use las_noches::bvh::Bvh;
use las_noches::camera::Camera;
use las_noches::geometry::Object;
use las_noches::image_io::save_png;
use las_noches::material::{Material, MATERIALS, OBSIDIAN, QUARTZ};
use las_noches::math::{Mat3, Vec3};
use las_noches::renderer::Quality;
use las_noches::scene::{Light, Scene};
use las_noches::texture::{Skybox, Texture};
use std::path::Path;

/// arma la escena de diagnostico, la renderiza desde dos camaras y guarda las dos
/// imagenes png en `out`. las texturas y el skybox se leen de `assets`.
pub fn run(assets: &Path, out: &Path) {
    // atajo para cargar una textura relativa a la carpeta de assets; si falta, paramos.
    let tex = |p: &str| Texture::load(&assets.join(p)).expect("falta textura de diagnóstico");
    // material del tablero: partimos del primer material de la lista y solo cambiamos
    // nombre, textura y albedo (la sintaxis `..` copia el resto de campos).
    let checker_m = Material { name: "Tablero", texture: "checker", albedo: 0.9, ..MATERIALS[0] };
    // material del cubo uv: sin brillo especular para que se vea limpia la textura.
    let uv_m = Material { name: "UV", texture: "uvtest", albedo: 0.9, specular: 0.0, ..MATERIALS[0] };
    // la posicion en esta lista es el indice que usa cada cubo:
    // 0 tablero, 1 uv, 2 cuarzo, 3 espejo (obsidiana con reflectividad muy alta y
    // albedo bajo, para que casi todo lo que se vea sea reflejo).
    let materials = vec![checker_m, uv_m, MATERIALS[QUARTZ as usize], Material { reflectivity: 0.85, albedo: 0.1, ..MATERIALS[OBSIDIAN as usize] }];
    // texturas en el mismo orden que los materiales de arriba.
    let textures = vec![
        tex("diag/checker.bmp"),
        tex("diag/uvtest.bmp"),
        tex("textures/quartz.bmp"),
        tex("textures/obsidian.bmp"),
    ];
    let mut o = Vec::new();
    // piso y pared de fondo con el tablero; la escala uv de 0.5 hace los cuadros mas
    // grandes para que los desplazamientos por refraccion se noten bien.
    o.push(Object::cube(Vec3::new(-12.0, -1.0, -8.0), Vec3::new(12.0, 0.0, 6.0), 0).with_uv_scale(0.5));
    o.push(Object::cube(Vec3::new(-12.0, 0.0, -8.0), Vec3::new(12.0, 8.0, -7.0), 0).with_uv_scale(0.5));
    // a: lamina de cuarzo de frente a la camara. con incidencia normal el rayo entra y
    // sale sin desviarse, entonces el tablero detras no debe verse desplazado.
    o.push(Object::cube(Vec3::new(-7.5, 0.0, -1.0), Vec3::new(-4.5, 5.0, -0.4), 2));
    // b: la misma lamina pero girada 40 grados en y alrededor de su centro `c`. ahora la
    // incidencia es oblicua y por snell el rayo sale paralelo pero corrido, asi que las
    // lineas del tablero vistas a traves deben aparecer desplazadas.
    let c = Vec3::new(-1.5, 2.5, -0.7);
    o.push(
        Object::cube(Vec3::new(-3.0, 0.0, -1.0), Vec3::new(0.0, 5.0, -0.4), 2).rotated(Mat3::rot_y(40f32.to_radians()), c),
    );
    // c: cubo de cuarzo girado en dos ejes (0.6 radianes en y, 0.5 en x). desde adentro
    // muchos rayos llegan a las caras con angulo mayor al critico (unos 40.5 grados para
    // cuarzo), asi que cerca de las aristas se ve reflexion interna total.
    let c2 = Vec3::new(3.0, 1.6, 0.5);
    o.push(
        Object::cube(c2 - Vec3::splat(1.3), c2 + Vec3::splat(1.3), 2)
            .rotated(Mat3::rot_y(0.6).mul(&Mat3::rot_x(0.5)), c2),
    );
    // d: placa delgada de obsidiana inclinada como espejo. sirve para comprobar que en
    // el reflejo se ven el tablero y tambien el cielo (el skybox en rayos reflejados).
    let c3 = Vec3::new(8.0, 2.5, -1.5);
    o.push(
        Object::cube(Vec3::new(6.5, 0.0, -1.7), Vec3::new(9.5, 5.0, -1.3), 3).rotated(Mat3::rot_y(-0.7).mul(&Mat3::rot_x(-0.45)), c3),
    );
    // e: cubo con la textura de prueba uv, girado un poco en y. en cada cara la letra f
    // debe leerse derecha y no como en espejo.
    let c4 = Vec3::new(-6.0, 1.0, 3.0);
    o.push(Object::cube(c4 - Vec3::splat(1.0), c4 + Vec3::splat(1.0), 1).rotated(Mat3::rot_y(0.6), c4));
    // construimos la jerarquia de volumenes envolventes para acelerar las intersecciones.
    let bvh = Bvh::build(&o);
    let scene = Scene {
        objects: o,
        bvh,
        materials,
        textures,
        skybox: Skybox::load(&assets.join("skybox")).unwrap(),
        // una sola luz direccional blanca y dura (spread 0, sin sombras suaves), para
        // que las sombras no distraigan de lo que queremos revisar.
        lights: vec![Light::Directional { dir: Vec3::new(0.3, 0.8, 0.5).normalized(), color: Vec3::splat(2.2), spread: 0.0 }],
        // luz ambiente gris un poco mas fuerte desde arriba que desde el suelo.
        ambient_sky: Vec3::splat(0.25),
        ambient_ground: Vec3::splat(0.15),
        // sin niebla, para que el fondo y los colores se vean tal cual.
        fog_density: 0.0,
        fog_color: Vec3::ZERO,
    };
    // profundidad de 8 rebotes: dentro del cubo de cuarzo un rayo puede rebotar varias
    // veces antes de salir, y con pocos niveles se veria negro.
    let q = Quality { max_depth: 8, soft_shadows: false };
    // dos vistas: una de frente (giro 0) y otra lateral (giro de 35 grados y camara mas alta).
    let views = [
        ("diag_frente.png", 0.0, Camera { target: Vec3::new(0.0, 2.5, 0.0), pitch: 0.15, distance: 20.0, fov_deg: 50.0 }),
        ("diag_lateral.png", 35.0, Camera { target: Vec3::new(0.0, 2.5, 0.0), pitch: 0.35, distance: 20.0, fov_deg: 50.0 }),
    ];
    for (name, yaw, cam) in views {
        // renderizamos a 960 por 540 con 4 muestras por pixel y guardamos en png.
        let img = render_image(&scene, yaw, cam, 960, 540, 4, q);
        let p = out.join(name);
        save_png(&p, &img).unwrap();
        // imprimimos la ruta para saber donde quedo cada imagen.
        println!("{}", p.display());
    }
}
