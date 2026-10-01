//! recorrido de cámara para el video de demostración.
//!
//! definimos unas cuantas claves de cámara (keyframes) y entre ellas interpolamos suavemente.
//! cada cuadro se renderiza y se guarda como un bmp numerado (frame_00000.bmp, frame_00001.bmp
//! y así), de forma determinista: el mismo número de cuadro siempre da la misma imagen.
//! el video se arma después juntando los cuadros con una herramienta externa (ffmpeg).

use crate::render_image;
use las_noches::camera::Camera;
use las_noches::image_io::save_bmp;
use las_noches::math::{smoothstep, Mat3, Vec3};
use las_noches::renderer::Quality;
use las_noches::scene::{Scene, PIVOT};
use std::path::Path;
use std::time::Instant;

/// configuración del render del video, se llena desde los argumentos en `main.rs`.
pub struct VideoSettings {
    /// ancho de cada cuadro en píxeles
    pub width: usize,
    /// alto de cada cuadro en píxeles
    pub height: usize,
    /// muestras por píxel
    pub spp: u32,
    /// cuadros por segundo del video
    pub fps: f32,
    /// primer cuadro que se renderiza
    pub start: usize,
    /// cuadro donde se detiene (sin incluirlo); `None` significa hasta el final
    pub end: Option<usize>,
}

/// clave de cámara. los campos en orden son: tiempo en segundos, rotación del diorama en grados,
/// inclinación en grados, distancia de la cámara al objetivo, y el punto al que mira la cámara
/// (en coordenadas del diorama, antes de girarlo).
#[derive(Clone, Copy)]
struct Key(f32, f32, f32, f32, [f32; 3]);

// puntos de interés del diorama a los que apunta la cámara durante el recorrido

/// objetivo de la vista héroe, el mismo de `hero_camera`.
const HERO: [f32; 3] = [0.0, 7.0, -4.0];
/// centro de la fortaleza, un poco más alto que el de la vista héroe.
const CENTER: [f32; 3] = [0.0, 8.0, -4.0];
/// el árbol de cuarzo del bosque.
const TREE: [f32; 3] = [-14.0, 4.5, 15.5];
/// el interior bajo la cúpula, visto por el corte.
const HALL: [f32; 3] = [-4.0, 6.0, -9.0];
/// la espada zangetsu clavada en la calzada.
const SWORD: [f32; 3] = [1.6, 1.25, 42.2];

/// lista de claves ordenadas por tiempo. cada par de claves seguidas forma un tramo del recorrido.
/// la rotación pasa de 360 grados en adelante a propósito: así el diorama sigue girando en el mismo
/// sentido en lugar de devolverse.
const KEYS: &[Key] = &[
    // al ras del suelo sobre la calzada de obsidiana: se ve el reflejo de la puerta y la cúpula
    Key(0.0, 0.0, -6.4, 50.0, HERO),
    Key(5.0, 0.0, -5.0, 45.0, HERO),
    // movimiento de grúa hacia arriba: la fortaleza completa entre la niebla
    Key(9.0, 0.0, 14.0, 62.0, CENTER),
    // rotación completa del diorama, una vuelta entera de 360 grados en 15 segundos
    Key(24.0, 360.0, 14.0, 62.0, CENTER),
    // el corte de la cúpula: el cielo falso del interior y su reflejo en el mármol pulido
    Key(30.0, 502.0, 20.0, 30.0, HALL),
    Key(35.0, 515.0, 26.0, 26.0, HALL),
    // árbol de cuarzo en primer plano: a través de él se ven refractadas las torres y la cúpula
    Key(40.0, 396.0, 4.0, 9.0, TREE),
    Key(44.0, 412.0, 6.0, 8.0, TREE),
    // zangetsu a escala real, clavada en la calzada, con su reflejo y las noches al fondo
    Key(49.0, 360.0, 6.0, 1.6, SWORD),
    Key(52.0, 372.0, 8.0, 1.25, SWORD),
    // nos alejamos mucho: el desierto infinito bajo la niebla y el skybox
    Key(56.5, 380.0, 22.0, 100.0, CENTER),
    // regreso a la vista héroe; 360 grados equivale a la rotación 0 del inicio, así el video cierra en bucle
    Key(60.0, 360.0, -6.4, 50.0, HERO),
];

/// duración total del recorrido en segundos: es el tiempo de la última clave.
pub fn duration() -> f32 {
    KEYS.last().unwrap().0
}

/// devuelve el giro del diorama en grados y la cámara para el instante `t` en segundos.
/// entre dos claves interpolamos con smoothstep, que arranca y frena suave en cada tramo
/// en vez de cambiar de velocidad de golpe como pasaría con una interpolación lineal.
pub fn camera_at(t: f32) -> (f32, Camera) {
    // buscamos el tramo: avanzamos mientras la siguiente clave ya haya pasado. el límite
    // `i + 2 < len` asegura que siempre exista la clave `i + 1`
    let mut i = 0;
    while i + 2 < KEYS.len() && KEYS[i + 1].0 <= t {
        i += 1;
    }
    let (a, b) = (KEYS[i], KEYS[i + 1]);
    // fracción del tramo recorrida, entre 0 y 1, y luego suavizada con smoothstep
    let s = smoothstep(0.0, 1.0, ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0));
    // interpolación lineal usando la fracción ya suavizada
    let lerp = |x: f32, y: f32| x + (y - x) * s;
    let yaw = lerp(a.1, b.1);
    // el objetivo está en coordenadas del diorama: lo interpolamos componente por componente
    // y después lo giramos alrededor del pivote con el mismo yaw para llevarlo al mundo
    let t_obj = Vec3::new(lerp(a.4[0], b.4[0]), lerp(a.4[1], b.4[1]), lerp(a.4[2], b.4[2]));
    let cam = Camera {
        target: Mat3::rot_y(yaw.to_radians()).apply(t_obj - PIVOT) + PIVOT,
        pitch: lerp(a.2, b.2).to_radians(),
        distance: lerp(a.3, b.3),
        fov_deg: 45.0,
    };
    (yaw, cam)
}

/// renderiza los cuadros del video en la carpeta `dir` según la configuración `s`.
/// si un cuadro ya existe en disco se lo salta, así que se puede cortar y volver a correr.
pub fn render_video(scene: &Scene, dir: &Path, s: &VideoSettings) {
    // número total de cuadros: duración por cuadros por segundo
    let total = (duration() * s.fps).round() as usize;
    // si piden un final más allá del total lo recortamos
    let end = s.end.unwrap_or(total).min(total);
    // calidad fija y alta para el video: 6 rebotes y sombras suaves
    let q = Quality { max_depth: 6, soft_shadows: true };
    std::fs::create_dir_all(dir).expect("no se pudo crear la carpeta de cuadros");
    let t0 = Instant::now();
    for f in s.start..end {
        // el nombre lleva el número con cinco dígitos para que se ordenen bien
        let path = dir.join(format!("frame_{f:05}.bmp"));
        if path.exists() {
            continue; // si el cuadro ya está hecho lo saltamos, así podemos reanudar donde quedó
        }
        // aquí no llamamos a `clamp`: el recorrido guionado puede acercarse más que el modo
        // interactivo porque revisamos cuadro por cuadro que la cámara no se mete en la geometría
        let (yaw, cam) = camera_at(f as f32 / s.fps);
        let img = render_image(scene, yaw, cam, s.width, s.height, s.spp, q);
        save_bmp(&path, &img).expect("no se pudo guardar el cuadro");
        // cada 10 cuadros mostramos el progreso: segundos por cuadro y estimado de minutos que faltan
        if f % 10 == 0 {
            let done = f + 1 - s.start;
            let el = t0.elapsed().as_secs_f32();
            println!("cuadro {f}/{total}  ({:.2} s/cuadro, faltan ~{:.0} min)", el / done as f32, el / done as f32 * (end - f - 1) as f32 / 60.0);
        }
    }
    println!("Listo: {} cuadros en {:.1} min", end - s.start, t0.elapsed().as_secs_f32() / 60.0);
}
