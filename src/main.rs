//! Punto de entrada.
//!
//!   cargo run --release                         -> ventana interactiva (rotar / zoom)
//!   cargo run --release -- --render out.png     -> render final a PNG
//!   cargo run --release -- --video frames/      -> cuadros BMP del recorrido del video
//!   cargo run --release -- --bench              -> mide tiempos por modo de calidad

mod diag;
mod video;
#[cfg(windows)]
mod window;

use las_noches::camera::Camera;
use las_noches::image_io::{save_bmp, save_png, Rgb8};
use las_noches::math::Vec3;
use las_noches::renderer::{render_accumulate, tonemap, Quality, Tracer, View};
use las_noches::scene::Scene;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const EXPOSURE: f32 = 1.35;

/// Vista "héroe": 3/4, ligeramente baja, mirando la torre y la luna a través del bosque.
pub const HERO_YAW_DEG: f32 = -28.0;
pub fn hero_camera() -> Camera {
    Camera { target: Vec3::new(0.0, 6.0, -1.0), pitch: 0.21, distance: 36.0, fov_deg: 45.0 }
}

pub struct Args {
    list: Vec<String>,
}
impl Args {
    fn has(&self, k: &str) -> bool {
        self.list.iter().any(|a| a == k)
    }
    fn get(&self, k: &str) -> Option<&str> {
        self.list.iter().position(|a| a == k).and_then(|i| self.list.get(i + 1)).map(|s| s.as_str())
    }
    fn num(&self, k: &str, d: f32) -> f32 {
        self.get(k).and_then(|s| s.parse().ok()).unwrap_or(d)
    }
}

/// Carpeta de assets relativa al proyecto (funciona con `cargo run` desde la raíz).
fn assets_dir() -> PathBuf {
    let here = Path::new("assets");
    if here.exists() {
        return here.to_path_buf();
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

pub fn load_scene() -> Scene {
    let dir = assets_dir();
    match Scene::load(&dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error cargando assets desde {}: {e}", dir.display());
            eprintln!("Genera las texturas con: cargo run --release --bin gen_textures");
            std::process::exit(1);
        }
    }
}

/// Render completo a un buffer RGB8.
pub fn render_image(scene: &Scene, yaw_deg: f32, cam: Camera, w: usize, h: usize, spp: u32, q: Quality) -> Rgb8 {
    let view = View::new(scene, yaw_deg.to_radians(), cam);
    let tracer = Tracer { scene, view: &view, q };
    let mut accum = vec![Vec3::ZERO; w * h];
    render_accumulate(&tracer, w, h, &mut accum, spp, 0);
    let mut img = Rgb8::new(w, h);
    tonemap(&accum, spp, w, h, EXPOSURE, &mut img.data);
    img
}

fn main() {
    let args = Args { list: std::env::args().skip(1).collect() };
    let scene = load_scene();

    let final_q = Quality { max_depth: args.num("--depth", 6.0) as u32, soft_shadows: true };
    let mut cam = hero_camera();
    cam.pitch = args.num("--pitch", cam.pitch.to_degrees()).to_radians();
    cam.distance = args.num("--dist", cam.distance);
    cam.clamp();
    let yaw = args.num("--yaw", HERO_YAW_DEG);

    if let Some(out) = args.get("--render") {
        let (w, h) = (args.num("--w", 1280.0) as usize, args.num("--h", 720.0) as usize);
        let spp = args.num("--spp", 16.0) as u32;
        let t = Instant::now();
        let img = render_image(&scene, yaw, cam, w, h, spp, final_q);
        let secs = t.elapsed().as_secs_f32();
        let path = Path::new(out);
        let res = if out.ends_with(".bmp") { save_bmp(path, &img) } else { save_png(path, &img) };
        res.expect("no se pudo guardar la imagen");
        println!("{out}: {w}x{h}, {spp} spp, profundidad {} -> {secs:.2} s", final_q.max_depth);
    } else if let Some(dir) = args.get("--video") {
        video::render_video(&scene, Path::new(dir), &args_to_video(&args));
    } else if let Some(dir) = args.get("--diag") {
        diag::run(&assets_dir(), Path::new(dir));
    } else if args.has("--bench") {
        bench(&scene);
    } else {
        #[cfg(windows)]
        window::run(&scene, args.has("--autotest"));
        #[cfg(not(windows))]
        eprintln!("La ventana interactiva usa la API de Windows; en otros sistemas usa --render.");
    }
}

fn args_to_video(a: &Args) -> video::VideoSettings {
    video::VideoSettings {
        width: a.num("--w", 1280.0) as usize,
        height: a.num("--h", 720.0) as usize,
        spp: a.num("--spp", 12.0) as u32,
        fps: a.num("--fps", 30.0),
        start: a.num("--from", 0.0) as usize,
        end: a.get("--to").and_then(|s| s.parse().ok()),
    }
}

fn bench(scene: &Scene) {
    let cam = hero_camera();
    let modes = [
        ("preview  320x180 1spp prof.2", 320, 180, 1, Quality { max_depth: 2, soft_shadows: false }),
        ("refinado 640x360 1spp prof.6", 640, 360, 1, Quality { max_depth: 6, soft_shadows: true }),
        ("final   1280x720 1spp prof.6", 1280, 720, 1, Quality { max_depth: 6, soft_shadows: true }),
    ];
    println!("Hilos: {}", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1));
    for (name, w, h, spp, q) in modes {
        let _ = render_image(scene, HERO_YAW_DEG, cam, w, h, spp, q); // calentamiento
        let runs = 3;
        let t = Instant::now();
        for _ in 0..runs {
            let _ = render_image(scene, HERO_YAW_DEG, cam, w, h, spp, q);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / runs as f64;
        println!("{name}: {ms:8.1} ms/cuadro ({:.1} fps)", 1000.0 / ms);
    }
}
