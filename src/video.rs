//! Recorrido de cámara para el video de demostración.
//! Renderiza cuadros BMP numerados (frame_00000.bmp, ...) de forma determinista;
//! el video se arma después con una herramienta externa de desarrollo (ffmpeg).

use crate::render_image;
use las_noches::camera::Camera;
use las_noches::image_io::save_bmp;
use las_noches::math::{smoothstep, Vec3};
use las_noches::renderer::Quality;
use las_noches::scene::Scene;
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

const CENTER: [f32; 3] = [0.0, 7.0, -1.0];
const BLADE: [f32; 3] = [6.6, 3.5, 2.0];
const DOME: [f32; 3] = [-2.0, 3.5, -2.0];
const TREE: [f32; 3] = [-8.0, 4.5, 7.0];

const KEYS: &[Key] = &[
    // Vista héroe con acercamiento lento.
    Key(0.0, -22.0, 11.0, 38.0, CENTER),
    Key(4.0, -22.0, 11.0, 33.0, CENTER),
    // Rotación completa del diorama (360°).
    Key(5.5, -14.0, 15.0, 37.0, CENTER),
    Key(19.0, 326.0, 15.0, 37.0, CENTER),
    // Zangetsu: reflexión en el acero.
    Key(23.0, 343.0, 10.0, 15.0, BLADE),
    Key(27.0, 353.0, 16.0, 14.0, BLADE),
    // La cúpula abierta: el "cielo falso" dentro de la noche eterna y su reflejo en la obsidiana.
    Key(31.0, 345.0, 28.0, 18.0, DOME),
    Key(35.5, 358.0, 22.0, 16.0, DOME),
    // Bosque de cuarzo: refracción de la torre, la cúpula y el cielo.
    Key(40.5, 409.0, 8.0, 15.0, TREE),
    Key(45.0, 425.0, 6.0, 13.0, TREE),
    // Alejarse: el diorama completo contra el skybox.
    Key(50.0, 440.0, 30.0, 52.0, CENTER),
    Key(54.0, 400.0, 20.0, 46.0, CENTER),
    // Regreso a la vista héroe.
    Key(58.0, 338.0, 11.0, 33.0, CENTER),
    Key(60.0, 338.0, 11.0, 33.0, CENTER),
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
    let cam = Camera {
        target: Vec3::new(lerp(a.4[0], b.4[0]), lerp(a.4[1], b.4[1]), lerp(a.4[2], b.4[2])),
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
