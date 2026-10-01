//! Recorrido de cámara para el video de demostración.
//! Renderiza cuadros BMP numerados (frame_00000.bmp, ...) de forma determinista;
//! el video se arma después con una herramienta externa de desarrollo (ffmpeg).

use crate::render_image;
use las_noches::camera::Camera;
use las_noches::image_io::save_bmp;
use las_noches::math::{smoothstep, Mat3, Vec3};
use las_noches::renderer::Quality;
use las_noches::scene::{Scene, PIVOT};
use std::path::Path;
use std::time::Instant;

pub struct VideoSettings {
    pub width: usize,
    pub height: usize,
    pub spp: u32,
    pub fps: f32,
    pub start: usize,
    pub end: Option<usize>,
}

/// Clave de cámara: tiempo (s), rotación del diorama (°), inclinación (°), distancia,
/// y punto al que mira la cámara (en coordenadas del diorama).
#[derive(Clone, Copy)]
struct Key(f32, f32, f32, f32, [f32; 3]);

const HERO: [f32; 3] = [0.0, 11.5, -4.0];
const CENTER: [f32; 3] = [0.0, 8.0, -4.0];
const TREE: [f32; 3] = [-14.0, 7.0, 15.5];
const MONOLITH: [f32; 3] = [-8.0, 2.4, 10.5];
const HALL: [f32; 3] = [-4.0, 6.0, -9.0];

const KEYS: &[Key] = &[
    // Vista héroe (composición de la referencia) con acercamiento lento.
    Key(0.0, -8.0, 5.0, 66.0, HERO),
    Key(4.0, -8.0, 5.0, 56.0, HERO),
    // Rotación completa del diorama (360°).
    Key(19.0, 352.0, 12.0, 62.0, CENTER),
    // Monolito de obsidiana: refleja el cielo detrás del espectador, donde está la Garganta.
    Key(23.0, 382.0, 14.0, 12.0, MONOLITH),
    Key(27.0, 374.0, 15.0, 11.0, MONOLITH),
    // Árbol de cuarzo en primer plano: refracción de las torres y la cúpula.
    Key(32.0, 396.0, 6.0, 15.0, TREE),
    Key(36.0, 410.0, 9.0, 14.0, TREE),
    // Corte de la cúpula: el cielo falso y su reflejo en el mármol pulido.
    Key(41.0, 503.0, 20.0, 30.0, HALL),
    Key(45.0, 515.0, 26.0, 26.0, HALL),
    // Alejarse: el diorama completo contra el skybox.
    Key(50.0, 540.0, 28.0, 88.0, CENTER),
    Key(54.0, 600.0, 16.0, 76.0, CENTER),
    // Regreso a la vista héroe.
    Key(58.0, 712.0, 5.0, 56.0, HERO),
    Key(60.0, 712.0, 5.0, 56.0, HERO),
];

pub fn duration() -> f32 {
    KEYS.last().unwrap().0
}

/// Interpolación suave entre claves (smoothstep por tramo).
pub fn camera_at(t: f32) -> (f32, Camera) {
    let mut i = 0;
    while i + 2 < KEYS.len() && KEYS[i + 1].0 <= t {
        i += 1;
    }
    let (a, b) = (KEYS[i], KEYS[i + 1]);
    let s = smoothstep(0.0, 1.0, ((t - a.0) / (b.0 - a.0)).clamp(0.0, 1.0));
    let lerp = |x: f32, y: f32| x + (y - x) * s;
    let yaw = lerp(a.1, b.1);
    // El objetivo está en coordenadas del diorama: se gira junto con él al mundo.
    let t_obj = Vec3::new(lerp(a.4[0], b.4[0]), lerp(a.4[1], b.4[1]), lerp(a.4[2], b.4[2]));
    let cam = Camera {
        target: Mat3::rot_y(yaw.to_radians()).apply(t_obj - PIVOT) + PIVOT,
        pitch: lerp(a.2, b.2).to_radians(),
        distance: lerp(a.3, b.3),
        fov_deg: 45.0,
    };
    (yaw, cam)
}

pub fn render_video(scene: &Scene, dir: &Path, s: &VideoSettings) {
    let total = (duration() * s.fps).round() as usize;
    let end = s.end.unwrap_or(total).min(total);
    let q = Quality { max_depth: 6, soft_shadows: true };
    std::fs::create_dir_all(dir).expect("no se pudo crear la carpeta de cuadros");
    let t0 = Instant::now();
    for f in s.start..end {
        let path = dir.join(format!("frame_{f:05}.bmp"));
        if path.exists() {
            continue; // permite reanudar
        }
        // Sin `clamp`: el recorrido guionado puede acercarse más que el modo interactivo
        // porque se verificó cuadro por cuadro que la cámara no entra en la geometría.
        let (yaw, cam) = camera_at(f as f32 / s.fps);
        let img = render_image(scene, yaw, cam, s.width, s.height, s.spp, q);
        save_bmp(&path, &img).expect("no se pudo guardar el cuadro");
        if f % 10 == 0 {
            let done = f + 1 - s.start;
            let el = t0.elapsed().as_secs_f32();
            println!("cuadro {f}/{total}  ({:.2} s/cuadro, faltan ~{:.0} min)", el / done as f32, el / done as f32 * (end - f - 1) as f32 / 60.0);
        }
    }
    println!("Listo: {} cuadros en {:.1} min", end - s.start, t0.elapsed().as_secs_f32() / 60.0);
}
