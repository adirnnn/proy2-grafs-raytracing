//! pruebas numericas de las partes delicadas del raytracer: refraccion con snell,
//! reflexion interna total, limites de fresnel, interseccion con cubos, orientacion de
//! las uv, ida y vuelta del skybox y que la escena no choque con la camara.
//! se corren con `cargo test`.

use las_noches::geometry::{intersect_cube, Ray};
use las_noches::math::Vec3;
use las_noches::renderer::refract;
use las_noches::texture::{dir_to_face, face_dir};

/// tolerancia para comparar flotantes (una diezmilesima).
const EPS: f32 = 1e-4;

/// un rayo que entra perpendicular al cuarzo no se desvia, y su reflectancia es la de
/// fresnel a incidencia normal.
#[test]
fn incidencia_normal_no_desvia() {
    // el rayo baja por menos z y la normal de la superficie apunta a mas z (hacia el rayo).
    let d = Vec3::new(0.0, 0.0, -1.0);
    let n = Vec3::new(0.0, 0.0, 1.0);
    // pasamos del aire (indice 1.0) al cuarzo (indice 1.54).
    let (t, f) = refract(d, n, 1.0, 1.54).unwrap();
    // con angulo de incidencia cero, snell da angulo de salida cero: misma direccion.
    assert!((t - d).length() < EPS, "a 0° el rayo no debe cambiar de dirección");
    // reflectancia de fresnel a incidencia normal: ((n1 menos n2) / (n1 mas n2)) al
    // cuadrado, que para cuarzo da (0.54 / 2.54) al cuadrado, mas o menos 4.5 por ciento.
    // schlick en coseno 1 da exactamente ese valor, por eso la tolerancia es tan chica.
    assert!((f - ((1.54f32 - 1.0) / 2.54).powi(2)).abs() < 1e-4);
}

/// a 40 grados el rayo transmitido cumple la ley de snell.
#[test]
fn ley_de_snell_oblicua() {
    // rayo que llega con 40 grados respecto a la normal, inclinado en el eje x.
    let ang = 40f32.to_radians();
    let d = Vec3::new(ang.sin(), 0.0, -ang.cos());
    let n = Vec3::new(0.0, 0.0, 1.0);
    let (t, _) = refract(d, n, 1.0, 1.54).unwrap();
    // snell dice n1 por seno de theta1 igual a n2 por seno de theta2. como `t` esta
    // normalizado y la normal es el eje z, su componente x es justo el seno de theta2.
    assert!((1.0 * ang.sin() - 1.54 * t.x).abs() < 1e-4);
    // ademas el rayo debe seguir entrando al material (z negativa), no rebotar.
    assert!(t.z < 0.0, "el rayo transmitido sigue avanzando hacia adentro");
}

/// desde dentro del cuarzo, pasado el angulo critico no hay rayo transmitido.
#[test]
fn reflexion_interna_total() {
    // el angulo critico del cuarzo es arcoseno de 1 entre 1.54, unos 40.5 grados.
    // a 60 grados desde adentro el seno de theta2 seria 1.54 por seno de 60, mas o menos
    // 1.33, que es mayor que 1, asi que hay reflexion interna total y no hay refraccion.
    let ang = 60f32.to_radians();
    let d = Vec3::new(ang.sin(), 0.0, ang.cos()); // sale por la cara de mas z
    let n_inside = Vec3::new(0.0, 0.0, -1.0); // normal del lado de donde viene el rayo
    assert!(refract(d, n_inside, 1.54, 1.0).is_none());
    // a 30 grados el seno de theta2 es 1.54 por 0.5, o sea 0.77, menor que 1: si sale.
    let ang = 30f32.to_radians();
    let d = Vec3::new(ang.sin(), 0.0, ang.cos());
    assert!(refract(d, n_inside, 1.54, 1.0).is_some());
}

/// la reflectancia de fresnel siempre esta entre 0 y 1 para todos los angulos.
#[test]
fn energia_fresnel_acotada() {
    // recorremos de 0 a 89 grados entrando del aire al cuarzo (nunca hay reflexion
    // interna total en ese sentido, por eso el unwrap es seguro).
    for deg in 0..90 {
        let a = (deg as f32).to_radians();
        let d = Vec3::new(a.sin(), 0.0, -a.cos());
        let (_, f) = refract(d, Vec3::new(0.0, 0.0, 1.0), 1.0, 1.54).unwrap();
        // si f saliera de [0, 1] se crearia o perderia energia entre reflejo y refraccion.
        assert!((0.0..=1.0).contains(&f));
    }
}

/// un rayo contra un cubo reporta bien la entrada desde afuera y la salida desde adentro.
#[test]
fn cubo_entrada_y_salida() {
    // cubo de lado 2 centrado en el origen.
    let (min, max) = (Vec3::splat(-1.0), Vec3::splat(1.0));
    // desde z igual a 5 mirando hacia menos z: choca con la cara z igual a 1, a distancia 4.
    let r = Ray { origin: Vec3::new(0.0, 0.0, 5.0), dir: Vec3::new(0.0, 0.0, -1.0) };
    let h = intersect_cube(&r, min, max, 1e-3, 1e9, 1.0).unwrap();
    assert!((h.t - 4.0).abs() < EPS);
    // la normal de esa cara apunta hacia mas z.
    assert!((h.normal - Vec3::new(0.0, 0.0, 1.0)).length() < EPS);
    // desde adentro (origen en el centro) debe reportar la salida por la cara z igual a
    // menos 1, a distancia 1, con la normal hacia afuera (menos z). esto es clave para
    // los rayos que viajan dentro del cuarzo.
    let r = Ray { origin: Vec3::ZERO, dir: Vec3::new(0.0, 0.0, -1.0) };
    let h = intersect_cube(&r, min, max, 1e-3, 1e9, 1.0).unwrap();
    assert!((h.t - 1.0).abs() < EPS);
    assert!((h.normal - Vec3::new(0.0, 0.0, -1.0)).length() < EPS);
}

/// para cada cara lateral vista desde afuera, u debe crecer hacia la derecha del
/// observador y v hacia arriba, es decir, las texturas no salen en espejo.
#[test]
fn uv_de_caras_sin_espejo() {
    // cubo unitario y su centro.
    let (min, max) = (Vec3::splat(0.0), Vec3::splat(1.0));
    let c = Vec3::splat(0.5);
    let up = Vec3::new(0.0, 1.0, 0.0);
    // probamos las cuatro caras laterales (mas x, menos x, mas z, menos z).
    for n in [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 0.0, -1.0),
    ] {
        // el observador mira hacia la cara, o sea en direccion contraria a su normal,
        // y su derecha es el producto cruz de adelante con arriba.
        let fwd = -n;
        let right = fwd.cross(up);
        // lanza un rayo desde 3 unidades frente a la cara, corrido por `off`.
        let hit_at = |off: Vec3| {
            let o = c + n * 3.0 + off;
            intersect_cube(&Ray { origin: o, dir: fwd }, min, max, 1e-3, 1e9, 1.0).unwrap()
        };
        // un golpe en el centro, otro un poco a la derecha y otro un poco arriba.
        let a = hit_at(Vec3::ZERO);
        let b = hit_at(right * 0.2);
        let u_up = hit_at(up * 0.2);
        // movernos a la derecha debe aumentar u y movernos arriba debe aumentar v.
        assert!(b.u > a.u, "u debe crecer a la derecha en la cara {n:?}");
        assert!(u_up.v > a.v, "v debe crecer hacia arriba en la cara {n:?}");
    }
}

/// pasar de (cara, u, v) a direccion y de vuelta a (cara, u, v) devuelve lo mismo.
#[test]
fn skybox_ida_y_vuelta() {
    // probamos las seis caras con tres puntos cada una (incluido el centro).
    for f in 0..6 {
        for (u, v) in [(0.1, 0.2), (0.5, 0.5), (0.9, 0.7)] {
            let d = face_dir(f, u, v).normalized();
            let (f2, u2, v2) = dir_to_face(d);
            // si las dos funciones son inversas, la cara es la misma y u, v casi iguales.
            // si alguna cara estuviera volteada este assert lo detectaria.
            assert_eq!(f, f2);
            assert!((u - u2).abs() < 1e-4 && (v - v2).abs() < 1e-4, "cara {f}");
        }
    }
}

/// todo lo que sobresale del suelo cabe dentro del radio minimo de zoom de la camara,
/// asi la camara nunca atraviesa un edificio.
#[test]
fn escena_dentro_de_limites() {
    use las_noches::scene::{build_diorama, GROUND_Y, PIVOT};
    let objs = build_diorama();
    // radio horizontal maximo medido desde el pivote alrededor del cual gira la camara.
    let mut r_max = 0.0f32;
    for o in &objs {
        let (a, b) = o.bounds();
        // se ignoran el suelo infinito (mas de 500 de ancho) y lo que casi no sobresale
        // del suelo (la calzada, o zangetsu que queda bajo la camara).
        if b.y <= GROUND_Y + 0.6 || b.x - a.x > 500.0 {
            continue;
        }
        // revisamos las cuatro esquinas de la caja en el plano xz y guardamos la mas lejana
        // usando la distancia euclidiana al pivote.
        for (x, z) in [(a.x, a.z), (a.x, b.z), (b.x, a.z), (b.x, b.z)] {
            r_max = r_max.max(((x - PIVOT.x).powi(2) + (z - PIVOT.z).powi(2)).sqrt());
        }
    }
    println!("objetos: {}  radio máximo de lo que sobresale: {r_max:.1}", objs.len());
    // a la distancia minima de zoom (y altura minima) la camara queda fuera de todo; el
    // factor 0.98 deja un margen del 2 por ciento por seguridad.
    assert!(r_max < las_noches::camera::MIN_DISTANCE * 0.98, "radio {r_max}");
}
