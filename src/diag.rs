//! Escenas de diagnóstico para verificar visualmente refracción, reflexión,
//! reflexión interna total, UV de las caras y el skybox en rayos secundarios.
//!   cargo run --release -- --diag carpeta/

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

pub fn run(assets: &Path, out: &Path) {
    let tex = |p: &str| Texture::load(&assets.join(p)).expect("falta textura de diagnóstico");
    let checker_m = Material { name: "Tablero", texture: "checker", albedo: 0.9, ..MATERIALS[0] };
    let uv_m = Material { name: "UV", texture: "uvtest", albedo: 0.9, specular: 0.0, ..MATERIALS[0] };
    // 0 tablero, 1 uv, 2 cuarzo, 3 espejo (obsidiana muy reflectiva)
    let materials = vec![checker_m, uv_m, MATERIALS[QUARTZ as usize], Material { reflectivity: 0.85, albedo: 0.1, ..MATERIALS[OBSIDIAN as usize] }];
    let textures = vec![
        tex("diag/checker.bmp"),
        tex("diag/uvtest.bmp"),
        tex("textures/quartz.bmp"),
        tex("textures/obsidian.bmp"),
    ];
    let mut o = Vec::new();
    // Piso y pared de fondo con tablero.
    o.push(Object::cube(Vec3::new(-12.0, -1.0, -8.0), Vec3::new(12.0, 0.0, 6.0), 0).with_uv_scale(0.5));
    o.push(Object::cube(Vec3::new(-12.0, 0.0, -8.0), Vec3::new(12.0, 8.0, -7.0), 0).with_uv_scale(0.5));
    // A: lámina de cuarzo de frente (incidencia normal: el tablero NO se desplaza).
    o.push(Object::cube(Vec3::new(-7.5, 0.0, -1.0), Vec3::new(-4.5, 5.0, -0.4), 2));
    // B: la misma lámina girada 40° (incidencia oblicua: el tablero se desplaza).
    let c = Vec3::new(-1.5, 2.5, -0.7);
    o.push(
        Object::cube(Vec3::new(-3.0, 0.0, -1.0), Vec3::new(0.0, 5.0, -0.4), 2).rotated(Mat3::rot_y(40f32.to_radians()), c),
    );
    // C: cubo de cuarzo girado (reflexión interna total en las aristas).
    let c2 = Vec3::new(3.0, 1.6, 0.5);
    o.push(
        Object::cube(c2 - Vec3::splat(1.3), c2 + Vec3::splat(1.3), 2)
            .rotated(Mat3::rot_y(0.6).mul(&Mat3::rot_x(0.5)), c2),
    );
    // D: espejo de obsidiana (reflexión del tablero y del cielo).
    let c3 = Vec3::new(8.0, 2.5, -1.5);
    o.push(
        Object::cube(Vec3::new(6.5, 0.0, -1.7), Vec3::new(9.5, 5.0, -1.3), 3).rotated(Mat3::rot_y(-0.7).mul(&Mat3::rot_x(-0.45)), c3),
    );
    // E: cubo UV (caras orientadas sin espejo).
    let c4 = Vec3::new(-6.0, 1.0, 3.0);
    o.push(Object::cube(c4 - Vec3::splat(1.0), c4 + Vec3::splat(1.0), 1).rotated(Mat3::rot_y(0.6), c4));
    let bvh = Bvh::build(&o);
    let scene = Scene {
        objects: o,
        bvh,
        materials,
        textures,
        skybox: Skybox::load(&assets.join("skybox")).unwrap(),
        lights: vec![Light::Directional { dir: Vec3::new(0.3, 0.8, 0.5).normalized(), color: Vec3::splat(2.2), spread: 0.0 }],
        ambient_sky: Vec3::splat(0.25),
        ambient_ground: Vec3::splat(0.15),
    };
    let q = Quality { max_depth: 8, soft_shadows: false };
    let views = [
        ("diag_frente.png", 0.0, Camera { target: Vec3::new(0.0, 2.5, 0.0), pitch: 0.15, distance: 20.0, fov_deg: 50.0 }),
        ("diag_lateral.png", 35.0, Camera { target: Vec3::new(0.0, 2.5, 0.0), pitch: 0.35, distance: 20.0, fov_deg: 50.0 }),
    ];
    for (name, yaw, cam) in views {
        let img = render_image(&scene, yaw, cam, 960, 540, 4, q);
        let p = out.join(name);
        save_png(&p, &img).unwrap();
        println!("{}", p.display());
    }
}
