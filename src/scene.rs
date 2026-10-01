//! construcción del diorama "las noches bajo la luna eterna".
//!
//! en este módulo armamos toda la escena: los materiales con sus texturas, el skybox,
//! las luces y la lista de objetos (todos son cubos) que después se ordenan en el `Bvh`.
//!
//! referencia: las noches en hueco mundo (bleach). es un tambor blanco enorme con una
//! cúpula baja, seis torres cilíndricas con base acampanada, desierto blanco, árboles
//! muertos de cuarzo y una luna creciente bajo un cielo nublado.
//!
//! escala: 1 unidad equivale aproximadamente a 4 m y el eje y apunta hacia arriba. el
//! suelo del desierto está en y = 1 y se extiende hasta perderse en la niebla, así que
//! no se ven bordes. la fortaleza mide unos 85 m de diámetro, sus torres llegan a unos
//! 115 m y zangetsu mide unos 2 m (0.53 unidades).

use crate::bvh::Bvh;
use crate::geometry::Object;
use crate::material::*;
use crate::math::{hash_u32, Mat3, Vec3};
use crate::texture::{Skybox, Texture};
use std::f32::consts::PI;
use std::io;
use std::path::Path;

/// tipos de luz que entiende el renderer.
/// usamos luces direccionales para la luna y luces puntuales para la puerta y la cúpula.
#[derive(Clone, Copy, Debug)]
pub enum Light {
    /// luz muy lejana (como la luna). `dir` apunta hacia la luz, no desde ella.
    /// `color` ya trae incluida la intensidad y `spread` es el tamaño angular
    /// aproximado de la fuente, que sirve para que las sombras salgan suaves.
    Directional { dir: Vec3, color: Vec3, spread: f32 },
    /// luz puntual en `pos`. su intensidad cae de forma suave a partir de `radius`
    /// y deja de aportar por completo a una distancia de 6 veces `radius`.
    Point { pos: Vec3, color: Vec3, radius: f32 },
}

/// todo lo que el renderer necesita para dibujar un cuadro.
pub struct Scene {
    /// lista plana de objetos del diorama (cubos con material).
    pub objects: Vec<Object>,
    /// jerarquía de cajas envolventes construida sobre `objects` para acelerar los rayos.
    pub bvh: Bvh,
    /// materiales en el mismo orden que la tabla `MATERIALS`.
    pub materials: Vec<Material>,
    /// una textura por material (mismo índice que en `materials`).
    pub textures: Vec<Texture>,
    /// cielo de fondo en forma de cubemap.
    pub skybox: Skybox,
    /// luces que proyectan sombra y brillo especular.
    pub lights: Vec<Light>,
    /// color ambiente que llega desde arriba (cielo nocturno).
    pub ambient_sky: Vec3,
    /// color ambiente que llega desde abajo (rebote de la arena).
    pub ambient_ground: Vec3,
    /// densidad de la niebla por unidad de distancia (0 significa sin niebla).
    pub fog_density: f32,
    /// color de la niebla: es el promedio del horizonte del skybox. así el suelo lejano
    /// se funde con el cielo y, como promediamos, las estrellas no dejan rayas.
    pub fog_color: Vec3,
}

/// dirección (en el mundo) hacia la luna dibujada en el skybox.
/// el generador de texturas usa la misma constante para pintar la luna en ese lugar,
/// por eso el contraluz coincide con la luna que se ve en el cielo.
pub const MOON_DIR: Vec3 = Vec3::new(0.02, 0.36, -0.93);
/// dirección de la garganta (la grieta roja) en el cielo; queda detrás de la cámara.
pub const GARGANTA_DIR: Vec3 = Vec3::new(0.18, 0.24, 0.95);
/// punto por el que pasa el eje vertical alrededor del cual gira el diorama
/// (el centro de las noches).
pub const PIVOT: Vec3 = Vec3::new(0.0, 0.0, -4.0);
/// altura del suelo plano del desierto.
pub const GROUND_Y: f32 = 1.0;

impl Scene {
    /// luz ambiente hemisférica para una normal `n`.
    /// si la normal apunta hacia arriba (n.y = 1) devolvemos el color del cielo, si apunta
    /// hacia abajo (n.y igual a menos 1) el de la arena, y en medio mezclamos linealmente.
    #[inline]
    pub fn ambient(&self, n: Vec3) -> Vec3 {
        // 0.5 + 0.5 * n.y pasa el rango de n.y (de menos 1 a 1) al rango de 0 a 1.
        self.ambient_ground.lerp(self.ambient_sky, 0.5 + 0.5 * n.y)
    }

    /// carga todos los recursos desde la carpeta `assets` y arma la escena completa.
    /// devuelve un error de entrada y salida si falta alguna textura o cara del skybox.
    pub fn load(assets: &Path) -> io::Result<Scene> {
        // copiamos la tabla constante de materiales a un vector propio de la escena.
        let materials: Vec<Material> = MATERIALS.to_vec();
        // cada material dice qué archivo bmp usar; lo buscamos en assets/textures.
        let mut textures = Vec::new();
        for m in &materials {
            textures.push(Texture::load(&assets.join("textures").join(format!("{}.bmp", m.texture)))?);
        }
        let skybox = Skybox::load(&assets.join("skybox"))?;
        // el color de la niebla sale del propio cielo para que el horizonte no tenga costura.
        let fog_color = horizon_color(&skybox);
        let objects = build_diorama();
        let bvh = Bvh::build(&objects);
        // posición de la luz de la puerta: un poco delante del frente del tambor.
        let gate = Vec3::new(DOME_C.x, 3.2, DOME_C.z + DRUM_R + 3.2);
        let lights = vec![
            // resplandor lunar difuso de las nubes: es la luz principal, tenue y con sombras suaves.
            Light::Directional {
                dir: Vec3::new(-0.45, 0.62, 0.64).normalized(),
                color: Vec3::new(0.70, 0.76, 0.88) * 0.38,
                spread: 0.05,
            },
            // contraluz de la luna (detrás de las noches): recorta las siluetas.
            Light::Directional { dir: MOON_DIR.normalized(), color: Vec3::new(0.80, 0.88, 1.0) * 0.9, spread: 0.02 },
            // el "cielo falso" de adentro: esta luz simula la claridad que se derrama por el corte de la cúpula.
            Light::Point { pos: DOME_C + Vec3::new(-2.5, 6.0, -2.5), color: Vec3::new(1.7, 2.3, 3.2) * 1.4, radius: 7.0 },
            // la puerta abierta: luz del interior que cae sobre la calzada.
            Light::Point { pos: gate, color: Vec3::new(1.5, 2.0, 2.7), radius: 3.5 },
        ];
        Ok(Scene {
            objects,
            bvh,
            materials,
            textures,
            skybox,
            lights,
            // ambientes muy bajos: es de noche y no queremos lavar las sombras.
            ambient_sky: Vec3::new(0.030, 0.034, 0.040),
            ambient_ground: Vec3::new(0.026, 0.026, 0.028),
            // con esta densidad lo que está a unos cientos de unidades ya es casi pura niebla.
            fog_density: 0.0065,
            fog_color,
        })
    }
}

/// promedio del color del cielo en un anillo justo sobre el horizonte.
/// recibe el skybox y devuelve un color lineal que usamos como color de la niebla.
pub fn horizon_color(sky: &Skybox) -> Vec3 {
    // tomamos 720 direcciones alrededor (una cada medio grado).
    let n = 720;
    let mut acc = Vec3::ZERO;
    for k in 0..n {
        // ángulo horizontal de esta muestra, de 0 a 2 pi.
        let a = k as f32 / n as f32 * 2.0 * PI;
        // tres alturas muy pequeñas sobre el horizonte para suavizar el promedio.
        for e in [0.01f32, 0.03, 0.05] {
            acc += sky.sample(Vec3::new(a.sin(), e, a.cos()).normalized());
        }
    }
    // dividimos entre el total de muestras (3 alturas por n ángulos).
    acc / (3 * n) as f32
}

// ============================================================================
// diseño del diorama
// ============================================================================

/// las celdas de terreno con dunas van desde menos `HALF` hasta `HALF` en x y en z;
/// fuera de ese cuadrado solo queda el suelo plano infinito.
pub const HALF: i32 = 44;
/// centro de las noches (punto en la base del tambor).
pub const DOME_C: Vec3 = Vec3::new(0.0, 1.0, -4.0);
/// radio exterior del tambor.
const DRUM_R: f32 = 10.5;
/// altura del tambor (la pared cilíndrica).
const DRUM_H: f32 = 8.0;
/// altura extra de la cúpula encima del tambor y la cornisa.
const DOME_H: f32 = 4.5;
/// tamaño de celda de la arquitectura: un cuarto de bloque, para que las curvas se vean suaves.
const CS: f32 = 0.25;
/// inicio del corte de maqueta, en grados alrededor del centro (0 es z positivo y
/// 90 es x positivo). el corte deja ver el interior.
const CUT_FROM: f32 = 180.0;
/// fin del corte de maqueta, en grados.
const CUT_TO: f32 = 255.0;
/// medio ancho de la calzada de obsidiana que va de la puerta hacia el frente.
const CAUSEWAY_HALF_W: f32 = 3.0;

/// ruido de valor en 2d: devuelve un número entre 0 y 1 que cambia suavemente con (x, z).
/// `seed` cambia el patrón para tener varias capas de ruido distintas.
fn noise2(x: f32, z: f32, seed: u32) -> f32 {
    // celda entera donde cae el punto y la parte fraccionaria dentro de esa celda.
    let (xi, zi) = (x.floor() as i32, z.floor() as i32);
    let (fx, fz) = (x - xi as f32, z - zi as f32);
    // valor pseudoaleatorio fijo para cada esquina de la rejilla: mezclamos las
    // coordenadas con dos primos grandes y la semilla, tomamos 16 bits y los pasamos a 0..1.
    let h = |a: i32, b: i32| {
        (hash_u32((a as u32).wrapping_mul(73_856_093) ^ (b as u32).wrapping_mul(19_349_663) ^ seed) & 0xFFFF) as f32
            / 65535.0
    };
    // curva smoothstep (3t al cuadrado menos 2t al cubo) para que no se noten las aristas de la rejilla.
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    // interpolamos en x sobre la fila de abajo (a) y la de arriba (b), y luego en z.
    let a = h(xi, zi) + (h(xi + 1, zi) - h(xi, zi)) * sx;
    let b = h(xi, zi + 1) + (h(xi + 1, zi + 1) - h(xi, zi + 1)) * sx;
    a + (b - a) * sz
}

/// distancia horizontal (en el plano xz) del punto (x, z) al centro de las noches.
fn dist_to_center(x: f32, z: f32) -> f32 {
    ((x - DOME_C.x).powi(2) + (z - DOME_C.z).powi(2)).sqrt()
}

/// ángulo en grados (de 0 a 360) de un punto alrededor del centro de las noches.
/// usamos atan2(dx, dz), así que 0 grados es z positivo y 90 grados es x positivo.
fn azimuth(x: f32, z: f32) -> f32 {
    // rem_euclid nos deja el ángulo siempre positivo aunque atan2 devuelva valores negativos.
    (x - DOME_C.x).atan2(z - DOME_C.z).to_degrees().rem_euclid(360.0)
}

/// dice si el punto (x, z) cae dentro de la rebanada que quitamos de la fortaleza
/// (entre `CUT_FROM` y `CUT_TO` grados). ahí no ponemos paredes ni cúpula.
fn in_cut(x: f32, z: f32) -> bool {
    let a = azimuth(x, z);
    a > CUT_FROM && a < CUT_TO
}

/// smoothstep de 0 a 1: recorta `t` a ese rango y lo suaviza en los extremos.
fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// dice si (x, z) está sobre la calzada: dentro de su ancho y delante del tambor.
fn on_causeway(x: f32, z: f32) -> bool {
    x.abs() <= CAUSEWAY_HALF_W && z > DOME_C.z + DRUM_R
}

/// altura de la arena en la celda (i, j). siempre devuelve múltiplos de medio bloque
/// por encima de `GROUND_Y`, para que las dunas se vean escalonadas.
pub fn sand_height(i: i32, j: i32) -> f32 {
    let (x, z) = (i as f32, j as f32);
    // dos capas de ruido: una grande y suave (pesa 0.65) y otra más fina (pesa 0.35).
    let n = noise2(x * 0.07, z * 0.07, 7) * 0.65 + noise2(x * 0.19, z * 0.19, 13) * 0.35;
    let r = dist_to_center(x, z);
    // las dunas forman un anillo alrededor de la fortaleza; son planas cerca de ella,
    // cerca de la calzada y lejos (donde se funden con el suelo infinito bajo la niebla).
    // `near` sube de 0 a 1 entre 2 y 10 unidades fuera del tambor.
    let near = smooth((r - DRUM_R - 2.0) / 8.0);
    // `far` vale 1 hasta un radio de 26 y baja a 0 hacia un radio de 40.
    let far = 1.0 - smooth((r - 26.0) / 14.0);
    // `path` aplana una franja alrededor de la calzada: el primer factor mide qué tan
    // cerca estamos en x de la calzada y el segundo solo actúa delante del tambor.
    let path = 1.0
        - (1.0 - smooth((x.abs() - CAUSEWAY_HALF_W - 0.5) / 5.0)) * smooth((z - DOME_C.z - DRUM_R + 2.0) / 3.0);
    // altura máxima de 4.5 unidades, atenuada por las tres máscaras.
    let h = n * 4.5 * near * far * path;
    // redondeamos hacia abajo a medios bloques: multiplicamos por 2, floor y dividimos entre 2.
    GROUND_Y + (h * 2.0).floor() * 0.5
}

/// atajo para crear un cubo a partir de su esquina mínima (x, y, z) y su tamaño
/// (sx, sy, sz) en cada eje, con el material `m`.
fn cube_at(x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32, m: u16) -> Object {
    Object::cube(Vec3::new(x, y, z), Vec3::new(x + sx, y + sy, z + sz), m)
}

/// construye la lista completa de objetos del diorama: suelo, dunas, las noches,
/// calzada, torres, árboles, lápidas, monolitos y zangetsu.
pub fn build_diorama() -> Vec<Object> {
    let mut o = Vec::new();

    // suelo del desierto: un cubo enorme (2000 unidades, unos 8 km de lado) que se pierde
    // en la niebla. empieza 6 unidades bajo cero y su cara superior queda en `GROUND_Y`.
    o.push(cube_at(-1000.0, -6.0, -1000.0, 2000.0, 6.0 + GROUND_Y, 2000.0, SAND));

    // dunas: una columna por celda donde la arena sube sobre el suelo.
    for i in -HALF..=HALF {
        for j in -HALF..=HALF {
            let (x, z) = (i as f32, j as f32);
            // no ponemos arena dentro de la huella de la fortaleza ni sobre la calzada.
            if dist_to_center(x, z) < DRUM_R + 0.5 || on_causeway(x, z) {
                continue;
            }
            let h = sand_height(i, j);
            if h > GROUND_Y {
                // la columna empieza medio bloque enterrada para que no quede ninguna rendija con el suelo.
                o.push(cube_at(x - 0.5, GROUND_Y - 0.5, z - 0.5, 1.0, h - GROUND_Y + 0.5, 1.0, SAND));
            }
        }
    }

    build_las_noches(&mut o);
    build_causeway(&mut o);

    // seis torres con base acampanada, cada una descrita como (x, z, radio, altura).
    // el índice k sirve de semilla para que cada torre tenga sus ventanas en otro ángulo.
    for (k, (x, z, r, h)) in [
        (-11.5, 5.5, 2.6, 27.0),
        (-15.0, -4.5, 2.3, 23.0),
        (-16.5, 3.0, 2.2, 14.0),
        (12.0, 5.0, 2.8, 29.0),
        (9.0, -16.0, 2.3, 25.0),
        (15.0, -7.0, 1.9, 17.0),
    ]
    .iter()
    .enumerate()
    {
        build_tower(&mut o, *x, *z, *r, *h, k as u32);
    }

    // árboles muertos de cuarzo dispersos (algunos lejos, entre la niebla).
    // cada entrada es (x, z, escala); la semilla es k más 1 para que todos sean distintos.
    for (k, (x, z, s)) in [
        (-14.0, 15.5, 1.7),
        (-21.0, 7.0, 1.0),
        (-8.0, 21.0, 0.8),
        (18.5, 15.0, 1.0),
        (21.0, -1.0, 0.85),
        (-21.0, -10.0, 0.95),
        (5.0, -21.5, 0.9),
        (12.5, 18.0, 1.35),
        (-30.0, 24.0, 1.2),
        (33.0, 20.0, 1.4),
        (-36.0, -8.0, 1.1),
        (28.0, -26.0, 1.3),
        (-9.0, 36.0, 0.9),
    ]
    .iter()
    .enumerate()
    {
        build_dead_tree(&mut o, Vec3::new(*x, 0.0, *z), k as u32 + 1, *s);
    }

    // lápidas de concreto junto a la calzada, descritas como (x, z, altura).
    for (x, z, h) in [(-5.0, 14.0, 3.4f32), (5.5, 11.0, 3.0), (-5.5, 22.0, 2.2), (6.0, 27.0, 2.6), (-5.5, 33.0, 1.8)] {
        // apoyamos la lápida sobre la arena de esa celda y la enterramos medio bloque.
        let g = sand_height(x as i32, z as i32);
        o.push(cube_at(x - 0.4, g - 0.5, z - 0.3, 0.8, h + 0.5, 0.6, STONE));
    }
    // monolitos de obsidiana pulida: (x, z, tamaño en x, y, z, giro en y, inclinación en x).
    // algunos van inclinados hacia atrás para que reflejen el cielo.
    for (x, z, sx, sy, sz, a, tilt) in [
        (17.0, 9.0, 2.2f32, 1.8f32, 1.6f32, 0.3f32, 0.0f32),
        (20.5, 4.0, 1.5, 1.5, 1.5, 0.8, 0.0),
        (-18.0, -17.0, 2.4, 2.0, 1.6, -0.4, 0.0),
        (-9.5, 12.0, 2.6, 3.0, 0.7, -0.5, -0.38),
    ] {
        let g = sand_height(x as i32, z as i32);
        // centro del bloque: sobre la arena, hundido 0.3 para que parezca clavado.
        let c = Vec3::new(x, g - 0.3 + sy * 0.5, z);
        // armamos el cubo centrado en c y lo rotamos alrededor de su propio centro.
        o.push(
            Object::cube(c - Vec3::new(sx, sy, sz) * 0.5, c + Vec3::new(sx, sy, sz) * 0.5, OBSIDIAN)
                .rotated(Mat3::rot_y(a).mul(&Mat3::rot_x(tilt)), c),
        );
    }

    // zangetsu, a escala humana, clavada en la arena junto a la calzada (en primer plano).
    build_zangetsu(&mut o, Vec3::new(1.6, GROUND_Y, 42.2));
    o
}

/// calzada de losas de obsidiana pulida con bordillo de concreto, desde la puerta
/// hasta el frente del diorama. agrega los cubos a `o`.
fn build_causeway(o: &mut Vec<Object>) {
    // empieza un poco delante del tambor y termina en z = 60.
    let z0 = DOME_C.z + DRUM_R + 1.8;
    let z1 = 60.0;
    let w = CAUSEWAY_HALF_W;
    let mut z = z0;
    while z < z1 {
        // losas de 2 bloques de largo, pegadas una a otra (la textura marca las juntas).
        // dejamos 0.3 libre a cada lado para el bordillo y la losa sobresale 0.06 del suelo.
        o.push(cube_at(-w + 0.3, GROUND_Y - 0.5, z, 2.0 * w - 0.6, 0.56, 2.0, OBSIDIAN).with_uv_scale(0.5));
        z += 2.0;
    }
    // bordillos: uno a cada lado (side vale menos 1 o 1), a lo largo de toda la calzada.
    for side in [-1.0f32, 1.0] {
        o.push(cube_at(side * w - 0.3, GROUND_Y - 0.5, z0, 0.6, 0.75, z1 - z0, STONE));
    }
}

/// las noches: tambor, cornisa, cúpula baja escalonada, cinco torrecillas, puerta
/// iluminada y un corte de maqueta que revela el cielo falso y el salón del trono.
/// todo se construye con columnas de cuarto de bloque y se agrega a `o`.
fn build_las_noches(o: &mut Vec<Object>) {
    let c = DOME_C;
    // punto de referencia dentro de la fortaleza, un poco arriba del piso.
    let inner = c + Vec3::new(0.0, 2.0, 0.0);
    // cuántas celdas de radio necesitamos para cubrir el tambor más la cornisa.
    let r = ((DRUM_R + 1.0) / CS).ceil() as i32;
    // las caras que miran hacia el interior (hacia `inner`) usan el material "cielo falso";
    // así, desde el corte, el techo y las paredes de adentro se ven como un cielo de día.
    let shell = |mut b: Object| {
        b.alt = Some((FAKE_SKY, inner));
        b
    };
    // altura de la cúpula a una distancia d del centro. el perfil es una elipse:
    // `DOME_H` por la raíz de (1 menos (d / `DRUM_R`) al cuadrado). el resultado lo
    // redondeamos a múltiplos de `CS`, y eso es lo que forma los escalones.
    // se suma encima del tambor y de la cornisa (0.7).
    let top = |d: f32| {
        c.y + DRUM_H + 0.7 + (DOME_H * (1.0 - (d / DRUM_R).powi(2)).max(0.0).sqrt() / CS).round() * CS
    };
    // una columna de `CS` por `CS` centrada en (x, z), desde y0 hasta y1, con material m.
    let cell = |x: f32, z: f32, y0: f32, y1: f32, m: u16| cube_at(x - CS * 0.5, y0, z - CS * 0.5, CS, y1 - y0, CS, m);
    // recorremos una rejilla cuadrada de celdas alrededor del centro.
    for di in -r..=r {
        for dj in -r..=r {
            let (x, z) = (c.x + di as f32 * CS, c.z + dj as f32 * CS);
            let d = dist_to_center(x, z);
            if d < DRUM_R - 1.0 {
                // piso interior de mármol pulido (también bajo el corte, para que se vea).
                o.push(cell(x, z, 0.0, c.y, MARBLE));
            }
            // dentro de la rebanada del corte no ponemos nada más.
            if in_cut(x, z) {
                continue;
            }
            // pared del tambor: un anillo de 1.2 de grosor, con una banda de ventanas
            // encendidas solo en la capa de celdas más externa.
            if d >= DRUM_R - 1.2 && d < DRUM_R {
                // cada 7.5 grados empieza una ventana que ocupa el 30 por ciento de ese tramo.
                let window = d >= DRUM_R - CS && (azimuth(x, z) / 7.5).fract() < 0.3;
                if window {
                    // partimos la columna en tres: muro abajo, vidrio encendido y muro arriba.
                    o.push(shell(cell(x, z, 0.0, 5.6, STONE)));
                    o.push(cell(x, z, 5.6, 6.3, LIGHT));
                    o.push(shell(cell(x, z, 6.3, c.y + DRUM_H, STONE)));
                } else {
                    o.push(shell(cell(x, z, 0.0, c.y + DRUM_H, STONE)));
                }
            }
            // cornisa que sobresale 0.7 del tambor, con 0.7 de alto, encima de la pared.
            if d >= DRUM_R - 1.2 && d < DRUM_R + 0.7 {
                o.push(shell(cell(x, z, c.y + DRUM_H, c.y + DRUM_H + 0.7, STONE)));
            }
            // cúpula escalonada: columnas macizas desde la parte alta del tambor hasta
            // `top(d)` (su cara inferior forma el techo del "cielo falso").
            if d < DRUM_R - 0.2 {
                o.push(shell(cell(x, z, c.y + DRUM_H, top(d), STONE)));
            }
        }
    }
    // altura de la cúpula en el centro (d = 0).
    let roof = c.y + DRUM_H + 0.7 + DOME_H;
    // cinco torrecillas sobre la cúpula, en fila sobre x; la del centro es la más alta
    // y cada paso hacia afuera pierde 0.4 de altura. se hunden 0.5 en el techo.
    for k in -2..=2 {
        let x = c.x + k as f32 * 1.1;
        let h = 2.6 - (k as f32).abs() * 0.4;
        o.push(cube_at(x - 0.35, roof - 0.5, c.z - 0.5, 0.7, h, 1.0, STONE));
    }
    // puerta frontal con el vano iluminado desde adentro.
    let front = c.z + DRUM_R;
    // pilar izquierdo y pilar derecho, que dejan un hueco de 1.8 en medio.
    o.push(cube_at(c.x - 2.8, 0.0, front - 1.5, 1.9, 5.5, 3.5, STONE));
    o.push(cube_at(c.x + 0.9, 0.0, front - 1.5, 1.9, 5.5, 3.5, STONE));
    // dintel que cierra el hueco por arriba.
    o.push(cube_at(c.x - 0.9, 4.2, front - 1.5, 1.8, 1.3, 3.5, STONE));
    // losa de remate que cubre los dos pilares.
    o.push(cube_at(c.x - 3.1, 5.5, front - 1.5, 6.2, 0.5, 3.8, STONE));
    // panel delgado emisivo dentro del vano: es la luz que se ve por la puerta abierta.
    o.push(cube_at(c.x - 0.9, 1.0, front - 1.4, 1.8, 3.2, 0.1, LIGHT));

    // interior: estrado de dos escalones y trono (asiento y respaldo alto).
    o.push(cube_at(c.x - 3.0, 1.0, c.z - 5.0, 6.0, 0.5, 4.0, STONE));
    o.push(cube_at(c.x - 2.0, 1.5, c.z - 4.6, 4.0, 0.5, 3.2, STONE));
    o.push(cube_at(c.x - 0.8, 2.0, c.z - 3.6, 1.6, 0.8, 1.4, STONE));
    o.push(cube_at(c.x - 0.8, 2.0, c.z - 4.2, 1.6, 4.5, 0.6, STONE));
    // columnas interiores: seis, separadas 60 grados, en un círculo de radio 6.5.
    for a in [0.0f32, 60.0, 120.0, 180.0, 240.0, 300.0] {
        // misma convención que `azimuth`: el seno va en x y el coseno en z.
        let (s, co) = a.to_radians().sin_cos();
        let (x, z) = (c.x + s * 6.5, c.z + co * 6.5);
        o.push(cube_at(x - 0.5, 1.0, z - 0.5, 1.0, DRUM_H, 1.0, STONE));
    }
}

/// torre cilíndrica de bloques con base acampanada y ventanas rasgadas encendidas.
/// parámetros: centro (`cx`, `cz`), radio de la parte alta `r0`, altura `h` y una
/// semilla para elegir el ángulo de las ventanas. los niveles consecutivos iguales de
/// una misma celda se unen en un solo bloque alto, así generamos muchos menos cubos.
fn build_tower(o: &mut Vec<Object>, cx: f32, cz: f32, r0: f32, h: f32, seed: u32) {
    const TS: f32 = 0.25; // cuarto de bloque por celda y por nivel
    // radio según la altura y: en la base vale r0 + 3 y decae exponencialmente hacia r0
    // (con escala 3.5); esa curva es la que da el acampanado de la base.
    let radius = |y: f32| r0 + 3.0 * (-y / 3.5).exp();
    // la torre arranca medio bloque bajo el suelo para que quede bien asentada.
    let base = GROUND_Y - 0.5;
    // celdas necesarias para cubrir el radio más ancho (el de la base) más una de margen.
    let rmax = (radius(0.0) / TS).ceil() as i32 + 1;
    // número de niveles de altura `TS` que caben en la torre.
    let levels = (h / TS) as i32;
    // ventanas: rendijas verticales a ciertas alturas y en un ángulo propio de cada torre
    // (sacado del hash de la semilla, entre 0 y 359 grados).
    let win_angle = (hash_u32(seed * 31 + 5) % 360) as f32;
    let is_window = |x: f32, z: f32, y: f32, d: f32, r: f32| {
        // solo en la capa más externa del muro y a partir de 8 unidades de altura.
        if d < r - TS || y < 8.0 {
            return false;
        }
        // ángulo de la celda alrededor del eje de la torre.
        let a = (x - cx).atan2(z - cz).to_degrees().rem_euclid(360.0);
        // diferencia angular con `win_angle`, llevada al rango de 0 a 180 grados
        // (sumar 540 y restar 180 resuelve el salto entre 359 y 0).
        let da = ((a - win_angle + 540.0) % 360.0 - 180.0).abs();
        // la altura se repite cada 5 unidades; la ventana ocupa la parte entre 0.15 y 0.55.
        let band = ((y - 8.0) / 5.0).fract();
        da < 7.0 && band > 0.15 && band < 0.55
    };
    for di in -rmax..=rmax {
        for dj in -rmax..=rmax {
            let (x, z) = (cx + di as f32 * TS, cz + dj as f32 * TS);
            let d = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
            // cada nivel es de un tipo: 0 = vacío, 1 = muro, 2 = ventana.
            // `run` guarda el tramo abierto como (nivel donde empezó, tipo).
            let mut run: Option<(i32, u8)> = None;
            for l in 0..=levels {
                // altura del centro del nivel l, medida desde la base de la torre.
                let y = (l as f32 + 0.5) * TS;
                let kind: u8 = if l == levels {
                    // un nivel extra vacío al final obliga a cerrar el último tramo.
                    0
                } else if l >= levels - 2 {
                    // los dos niveles de arriba son una tapa maciza (disco completo).
                    (d < radius(y)) as u8
                } else {
                    // el resto es un cascarón: muro de 0.75 de grosor por dentro del radio.
                    let r = radius(y);
                    if d < r && d >= r - 0.75 {
                        if is_window(x, z, y, d, r) { 2 } else { 1 }
                    } else {
                        0
                    }
                };
                match run {
                    // mismo tipo que el tramo abierto: solo lo alargamos (no hacemos nada).
                    Some((_, k)) if k == kind => {}
                    // cambió el tipo: emitimos un cubo desde el nivel s hasta el nivel l
                    // y abrimos un tramo nuevo si este nivel no está vacío.
                    Some((s, k)) => {
                        let (y0, y1) = (base + s as f32 * TS, base + l as f32 * TS);
                        let m = if k == 2 { LIGHT } else { STONE };
                        o.push(cube_at(x - TS * 0.5, y0, z - TS * 0.5, TS, y1 - y0, TS, m));
                        run = if kind > 0 { Some((l, kind)) } else { None };
                    }
                    // no había tramo y este nivel tiene algo: empezamos uno.
                    None if kind > 0 => run = Some((l, kind)),
                    // no había tramo y el nivel está vacío: seguimos.
                    None => {}
                }
            }
        }
    }
}

/// árbol muerto de cuarzo: tronco torcido de cuatro segmentos y ramas que se dividen dos veces.
/// `base` da la posición en x y z, `seed` hace que cada árbol sea distinto y `scale`
/// agranda o achica todo el árbol.
fn build_dead_tree(o: &mut Vec<Object>, base: Vec3, seed: u32, scale: f32) {
    // número pseudoaleatorio entre 0 y 1, fijo para cada pareja (semilla, k).
    let rnd = |k: u32| (hash_u32(seed * 977 + k) & 0xFFFF) as f32 / 65535.0;
    // el árbol nace en la arena de su celda, enterrado 0.4.
    let ground = sand_height(base.x.round() as i32, base.z.round() as i32);
    let mut p = Vec3::new(base.x, ground - 0.4, base.z);
    // `rot` acumula la orientación del tronco y `w` es su grosor actual.
    let mut rot = Mat3::IDENTITY;
    let mut w = 0.85 * scale;
    for s in 0..4 {
        // cada segmento gira al azar alrededor de y y se ladea un poco en z
        // (como mucho 0.175 radianes hacia cada lado); así el tronco queda torcido.
        rot = rot.mul(&Mat3::rot_y(rnd(s) * 2.0 * PI).mul(&Mat3::rot_z((rnd(s + 10) - 0.5) * 0.35)));
        // largo del segmento entre 2 y 3 veces la escala.
        let len = (2.0 + rnd(s + 20)) * scale;
        push_branch(o, p, rot, w, len);
        // avanzamos al extremo del segmento (al 95 por ciento, para que se traslapen y no haya huecos).
        p = p + rot.apply(Vec3::new(0.0, len * 0.95, 0.0));
        // cada segmento es un 18 por ciento más delgado que el anterior.
        w *= 0.82;
        // ramas laterales en los dos segmentos superiores.
        if s >= 2 {
            for b in 0..2u32 {
                let k = s * 10 + b;
                // rama con dirección al azar alrededor de y y abierta entre 0.5 y 1 radián.
                let br = Mat3::rot_y(rnd(30 + k) * 2.0 * PI).mul(&Mat3::rot_x(0.5 + rnd(40 + k) * 0.5));
                let l1 = (1.6 + rnd(50 + k) * 1.4) * scale;
                push_branch(o, p, br, w * 0.6, l1);
                // segunda división: desde la punta sale una ramita que se dobla hacia el otro lado.
                let tip = p + br.apply(Vec3::new(0.0, l1 * 0.95, 0.0));
                let br2 = br.mul(&Mat3::rot_x(-0.35 - rnd(60 + k) * 0.3));
                push_branch(o, tip, br2, w * 0.4, l1 * 0.6);
            }
        }
    }
    // copa: dos ramitas finales opuestas (separadas pi radianes alrededor de y).
    for b in 0..2u32 {
        let br = rot.mul(&Mat3::rot_y(b as f32 * PI + rnd(70)).mul(&Mat3::rot_x(0.4)));
        push_branch(o, p, br, w * 0.5, 1.4 * scale);
    }
}

/// agrega una rama: un prisma de sección cuadrada de lado `w` y largo `len` que crece
/// hacia arriba desde `start` y luego se rota con `rot` alrededor de ese mismo punto.
fn push_branch(o: &mut Vec<Object>, start: Vec3, rot: Mat3, w: f32, len: f32) {
    o.push(
        Object::cube(
            Vec3::new(start.x - w * 0.5, start.y, start.z - w * 0.5),
            Vec3::new(start.x + w * 0.5, start.y + len, start.z + w * 0.5),
            QUARTZ,
        )
        .rotated(rot, start)
        .with_uv_scale(0.6),
    );
}

/// zangetsu (shikai) a escala real (1 unidad equivale aproximadamente a 4 m): hoja de
/// unos 1.6 m por 0.4 m, sin guarda y con el mango vendado. queda clavada e inclinada
/// en la arena; `p` es el punto donde la hoja entra en el suelo.
pub fn build_zangetsu(o: &mut Vec<Object>, p: Vec3) {
    // inclinación de la espada: giro en y, se echa un poco hacia atrás en x y se ladea en z.
    let rot = Mat3::rot_y(0.35).mul(&Mat3::rot_x(-0.2)).mul(&Mat3::rot_z(0.12));
    // ancho de la hoja (0.4 m), largo de la hoja (1.6 m), grosor y parte enterrada.
    let blade_w = 0.10;
    let blade_l = 0.40;
    let t = 0.012;
    let sunk = 0.08;
    // hoja de acero; la textura se repite 4 veces porque la pieza es muy pequeña.
    o.push(
        Object::cube(
            Vec3::new(p.x - blade_w * 0.5, p.y - sunk, p.z - t * 0.5),
            Vec3::new(p.x + blade_w * 0.5, p.y + blade_l - sunk, p.z + t * 0.5),
            STEEL,
        )
        .rotated(rot, p)
        .with_uv_scale(4.0),
    );
    // lomo oscuro: una tira de obsidiana pegada al borde de la hoja en x negativo,
    // un poco más gruesa y un poco más alta que la hoja.
    o.push(
        Object::cube(
            Vec3::new(p.x - blade_w * 0.5 - 0.012, p.y - sunk, p.z - t * 0.7),
            Vec3::new(p.x - blade_w * 0.5, p.y + blade_l - sunk + 0.01, p.z + t * 0.7),
            OBSIDIAN,
        )
        .rotated(rot, p),
    );
    // mango vendado: barra delgada encima de la hoja (unos 0.5 m de largo).
    o.push(
        Object::cube(
            Vec3::new(p.x - 0.014, p.y + blade_l - sunk, p.z - 0.014),
            Vec3::new(p.x + 0.014, p.y + blade_l - sunk + 0.13, p.z + 0.014),
            HILT,
        )
        .rotated(rot, p)
        .with_uv_scale(20.0),
    );
}
