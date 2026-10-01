//! Pruebas numéricas de las partes delicadas del raytracer.

use las_noches::geometry::{intersect_cube, Ray};
use las_noches::math::Vec3;
use las_noches::renderer::refract;
use las_noches::texture::{dir_to_face, face_dir};

const EPS: f32 = 1e-4;

#[test]
fn incidencia_normal_no_desvia() {
    let d = Vec3::new(0.0, 0.0, -1.0);
    let n = Vec3::new(0.0, 0.0, 1.0);
    let (t, f) = refract(d, n, 1.0, 1.54).unwrap();
    assert!((t - d).length() < EPS, "a 0° el rayo no debe cambiar de dirección");
    // Reflectancia de Fresnel a incidencia normal: ((n1-n2)/(n1+n2))^2 ≈ 4.5% para cuarzo.
    assert!((f - ((1.54f32 - 1.0) / 2.54).powi(2)).abs() < 1e-4);
}

#[test]
fn ley_de_snell_oblicua() {
    let ang = 40f32.to_radians();
    let d = Vec3::new(ang.sin(), 0.0, -ang.cos());
    let n = Vec3::new(0.0, 0.0, 1.0);
    let (t, _) = refract(d, n, 1.0, 1.54).unwrap();
    // n1 sin θ1 = n2 sin θ2
    assert!((1.0 * ang.sin() - 1.54 * t.x).abs() < 1e-4);
    assert!(t.z < 0.0, "el rayo transmitido sigue avanzando hacia adentro");
}

#[test]
fn reflexion_interna_total() {
    // Ángulo crítico del cuarzo: asin(1/1.54) ≈ 40.5°. A 60° desde adentro => TIR.
    let ang = 60f32.to_radians();
    let d = Vec3::new(ang.sin(), 0.0, ang.cos()); // saliendo por la cara +z
    let n_inside = Vec3::new(0.0, 0.0, -1.0); // normal del lado del rayo
    assert!(refract(d, n_inside, 1.54, 1.0).is_none());
    // A 30° sí sale.
    let ang = 30f32.to_radians();
    let d = Vec3::new(ang.sin(), 0.0, ang.cos());
    assert!(refract(d, n_inside, 1.54, 1.0).is_some());
}

#[test]
fn energia_fresnel_acotada() {
    for deg in 0..90 {
        let a = (deg as f32).to_radians();
        let d = Vec3::new(a.sin(), 0.0, -a.cos());
        let (_, f) = refract(d, Vec3::new(0.0, 0.0, 1.0), 1.0, 1.54).unwrap();
        assert!((0.0..=1.0).contains(&f));
    }
}

#[test]
fn cubo_entrada_y_salida() {
    let (min, max) = (Vec3::splat(-1.0), Vec3::splat(1.0));
    let r = Ray { origin: Vec3::new(0.0, 0.0, 5.0), dir: Vec3::new(0.0, 0.0, -1.0) };
    let h = intersect_cube(&r, min, max, 1e-3, 1e9, 1.0).unwrap();
    assert!((h.t - 4.0).abs() < EPS);
    assert!((h.normal - Vec3::new(0.0, 0.0, 1.0)).length() < EPS);
    // Desde adentro: debe reportar la salida con normal hacia afuera.
    let r = Ray { origin: Vec3::ZERO, dir: Vec3::new(0.0, 0.0, -1.0) };
    let h = intersect_cube(&r, min, max, 1e-3, 1e9, 1.0).unwrap();
    assert!((h.t - 1.0).abs() < EPS);
    assert!((h.normal - Vec3::new(0.0, 0.0, -1.0)).length() < EPS);
}

/// Para cada cara vista desde afuera, u debe crecer hacia la derecha del observador
/// y v hacia arriba (texturas sin espejo).
#[test]
fn uv_de_caras_sin_espejo() {
    let (min, max) = (Vec3::splat(0.0), Vec3::splat(1.0));
    let c = Vec3::splat(0.5);
    let up = Vec3::new(0.0, 1.0, 0.0);
    for n in [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, -1.0),
    ] {
        let fwd = -n;
        let right = fwd.cross(up);
        let hit_at = |off: Vec3| {
            let o = c + n * 3.0 + off;
            intersect_cube(&Ray { origin: o, dir: fwd }, min, max, 1e-3, 1e9, 1.0).unwrap()
        };
        let a = hit_at(Vec3::ZERO);
        let b = hit_at(right * 0.2);
        let u_up = hit_at(up * 0.2);
        assert!(b.u > a.u, "u debe crecer a la derecha en la cara {n:?}");
        assert!(u_up.v > a.v, "v debe crecer hacia arriba en la cara {n:?}");
    }
}

#[test]
fn skybox_ida_y_vuelta() {
    for f in 0..6 {
        for (u, v) in [(0.1, 0.2), (0.5, 0.5), (0.9, 0.7)] {
            let d = face_dir(f, u, v).normalized();
            let (f2, u2, v2) = dir_to_face(d);
            assert_eq!(f, f2);
            assert!((u - u2).abs() < 1e-4 && (v - v2).abs() < 1e-4, "cara {f}");
        }
    }
}

#[test]
fn escena_dentro_de_limites() {
    use las_noches::scene::build_diorama;
    let objs = build_diorama();
    let mut cubes = 0;
    let mut hmin = Vec3::splat(f32::INFINITY);
    let mut hmax = Vec3::splat(f32::NEG_INFINITY);
    for o in &objs {
        if matches!(o.shape, las_noches::geometry::Shape::Cube { .. }) {
            cubes += 1;
        }
        let (a, b) = o.bounds();
        hmin = hmin.min(a);
        hmax = hmax.max(b);
    }
    println!("objetos: {}  cubos: {}  límites: {:?} .. {:?}", objs.len(), cubes, hmin, hmax);
    // La distancia mínima de zoom deja la cámara fuera de todo el diorama
    // (la cámara interactiva mira siempre a un punto sobre el pivote).
    // Radio horizontal máximo medido desde el pivote de rotación.
    let p = las_noches::scene::PIVOT;
    let r = ((hmax.x - p.x).abs().max((hmin.x - p.x).abs()).powi(2) + (hmax.z - p.z).abs().max((hmin.z - p.z).abs()).powi(2)).sqrt();
    assert!(r < las_noches::camera::MIN_DISTANCE, "radio horizontal {r}");
}
