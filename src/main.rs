//! punto de entrada del programa.
//!
//! según los argumentos que reciba, el programa hace una de estas cosas:
//!   sin argumentos: abre la ventana interactiva (rotar, inclinar y hacer zoom)
//!   con la opción render seguida de un archivo: hace un render final y lo guarda como png o bmp
//!   con la opción video seguida de una carpeta: genera los cuadros bmp del recorrido del video
//!   con la opción diag seguida de una carpeta: genera imágenes de diagnóstico
//!   con la opción bench: mide cuánto tarda un cuadro en cada modo de calidad
//!
//! además se pueden ajustar la cámara (yaw, pitch, dist, target) y la calidad (w, h, spp, depth).
//! conviene correrlo siempre en modo release porque en modo debug el raytracer es muchísimo más lento.

mod diag;
mod video;
// la ventana usa la api de windows, así que ese módulo solo se compila en windows
#[cfg(windows)]
mod window;

use las_noches::camera::Camera;
use las_noches::image_io::{save_bmp, save_png, Rgb8};
use las_noches::math::Vec3;
use las_noches::renderer::{render_accumulate, tonemap, Quality, Tracer, View};
use las_noches::scene::Scene;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// exposición que usamos al pasar de color lineal a bytes: valores mayores a 1 aclaran la imagen.
pub const EXPOSURE: f32 = 1.35;

/// giro inicial del diorama en grados para la vista héroe.
pub const HERO_YAW_DEG: f32 = 0.0;
/// cámara de la vista "héroe": de tres cuartos, un poco baja, mirando la torre y la luna a través del bosque.
/// la inclinación va en radianes; el valor negativo pone la cámara apenas por debajo del objetivo.
pub fn hero_camera() -> Camera {
    Camera { target: Vec3::new(0.0, 7.0, -4.0), pitch: -0.112, distance: 50.0, fov_deg: 45.0 }
}

/// argumentos de la línea de comandos guardados como una lista de textos.
/// no usamos ninguna librería para esto, buscamos las opciones a mano.
pub struct Args {
    list: Vec<String>,
}
impl Args {
    /// dice si la opción `k` aparece en la lista (para opciones sin valor, como la de bench).
    fn has(&self, k: &str) -> bool {
        self.list.iter().any(|a| a == k)
    }
    /// busca la opción `k` y devuelve el texto que viene justo después, si existe.
    fn get(&self, k: &str) -> Option<&str> {
        self.list.iter().position(|a| a == k).and_then(|i| self.list.get(i + 1)).map(|s| s.as_str())
    }
    /// igual que `get` pero lo convierte a número; si falta o no se puede leer devolvemos el valor por defecto `d`.
    fn num(&self, k: &str, d: f32) -> f32 {
        self.get(k).and_then(|s| s.parse().ok()).unwrap_or(d)
    }
}

/// carpeta de assets. primero probamos con la carpeta relativa al directorio actual (funciona con
/// `cargo run` desde la raíz) y si no existe usamos la ruta del proyecto que cargo fija al compilar.
fn assets_dir() -> PathBuf {
    let here = Path::new("assets");
    if here.exists() {
        return here.to_path_buf();
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

/// carga la escena completa (texturas, skybox, objetos). si faltan los archivos explicamos cómo
/// generarlos y cerramos el programa, porque sin escena no hay nada que renderizar.
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

/// render completo de una imagen a un buffer rgb de 8 bits.
/// `yaw_deg` es el giro del diorama en grados, `spp` las muestras por píxel y `q` la calidad.
pub fn render_image(scene: &Scene, yaw_deg: f32, cam: Camera, w: usize, h: usize, spp: u32, q: Quality) -> Rgb8 {
    // la vista combina el giro del diorama con la cámara
    let view = View::new(scene, yaw_deg.to_radians(), cam);
    let tracer = Tracer { scene, view: &view, q };
    // aquí se suman las `spp` muestras de cada píxel
    let mut accum = vec![Vec3::ZERO; w * h];
    render_accumulate(&tracer, w, h, &mut accum, spp, 0);
    // tone mapping: promediamos dividiendo entre `spp`, aplicamos la exposición y pasamos a bytes
    let mut img = Rgb8::new(w, h);
    tonemap(&accum, spp, w, h, EXPOSURE, &mut img.data);
    img
}

/// lee los argumentos, prepara la cámara y decide qué modo ejecutar.
fn main() {
    // saltamos el primer argumento porque es el nombre del ejecutable
    let args = Args { list: std::env::args().skip(1).collect() };
    let scene = load_scene();

    // calidad para los renders finales: profundidad de rebotes configurable (6 por defecto) y sombras suaves
    let final_q = Quality { max_depth: args.num("--depth", 6.0) as u32, soft_shadows: true };
    // partimos de la cámara héroe y le aplicamos lo que venga por argumentos.
    // la inclinación se escribe en grados y la guardamos en radianes
    let mut cam = hero_camera();
    cam.pitch = args.num("--pitch", cam.pitch.to_degrees()).to_radians();
    cam.distance = args.num("--dist", cam.distance);
    cam.clamp();
    // punto al que mira la cámara, escrito como x,y,z (sirve para los acercamientos a materiales)
    if let Some(t) = args.get("--target") {
        let v: Vec<f32> = t.split(',').filter_map(|x| x.parse().ok()).collect();
        if v.len() == 3 {
            // las coordenadas vienen en el espacio del diorama, así que las giramos junto con él:
            // restamos el pivote, rotamos alrededor del eje y con el mismo yaw y volvemos a sumar el pivote
            let p = las_noches::scene::PIVOT;
            cam.target = las_noches::math::Mat3::rot_y(yaw_deg_arg(&args).to_radians()).apply(Vec3::new(v[0], v[1], v[2]) - p) + p;
            cam.distance = args.num("--dist", cam.distance); // volvemos a leer la distancia sin pasar por `clamp`, así los renders fijos pueden acercarse sin límite mínimo
        }
    }
    let yaw = args.num("--yaw", HERO_YAW_DEG);

    if let Some(out) = args.get("--render") {
        // modo render: una sola imagen con la resolución y muestras pedidas
        let (w, h) = (args.num("--w", 1280.0) as usize, args.num("--h", 720.0) as usize);
        let spp = args.num("--spp", 16.0) as u32;
        let t = Instant::now();
        let img = render_image(&scene, yaw, cam, w, h, spp, final_q);
        let secs = t.elapsed().as_secs_f32();
        let path = Path::new(out);
        // el formato se elige por la extensión del archivo de salida
        let res = if out.ends_with(".bmp") { save_bmp(path, &img) } else { save_png(path, &img) };
        res.expect("no se pudo guardar la imagen");
        println!("{out}: {w}x{h}, {spp} spp, profundidad {} -> {secs:.2} s", final_q.max_depth);
    } else if let Some(dir) = args.get("--video") {
        // modo video: los cuadros del recorrido de cámara en la carpeta indicada
        video::render_video(&scene, Path::new(dir), &args_to_video(&args));
    } else if let Some(dir) = args.get("--diag") {
        // modo diagnóstico
        diag::run(&assets_dir(), Path::new(dir));
    } else if args.has("--bench") {
        // modo benchmark: medición de tiempos
        bench(&scene);
    } else {
        // sin opciones especiales abrimos la ventana interactiva (o autotest si se pide)
        #[cfg(windows)]
        window::run(&scene, args.has("--autotest"));
        #[cfg(not(windows))]
        eprintln!("La ventana interactiva usa la API de Windows; en otros sistemas usa --render.");
    }
}

/// giro del diorama en grados leído de los argumentos (o el de la vista héroe si no viene).
/// lo usamos para girar el objetivo antes de leer `yaw` en `main`.
fn yaw_deg_arg(a: &Args) -> f32 {
    a.num("--yaw", HERO_YAW_DEG)
}

/// arma la configuración del video a partir de los argumentos, con valores por defecto
/// de 1280 por 720, 12 muestras por píxel y 30 cuadros por segundo.
fn args_to_video(a: &Args) -> video::VideoSettings {
    video::VideoSettings {
        width: a.num("--w", 1280.0) as usize,
        height: a.num("--h", 720.0) as usize,
        spp: a.num("--spp", 12.0) as u32,
        fps: a.num("--fps", 30.0),
        // primer cuadro a renderizar, útil para repartir el trabajo o retomar
        start: a.num("--from", 0.0) as usize,
        // último cuadro (sin incluir); si no viene se renderiza hasta el final
        end: a.get("--to").and_then(|s| s.parse().ok()),
    }
}

/// benchmark: mide cuánto tarda en promedio un cuadro con distintas resoluciones y calidades.
fn bench(scene: &Scene) {
    let cam = hero_camera();
    // cada modo es (nombre, ancho, alto, muestras por píxel, calidad)
    let modes = [
        ("preview  320x180 1spp prof.2", 320, 180, 1, Quality { max_depth: 2, soft_shadows: false }),
        ("refinado 640x360 1spp prof.6", 640, 360, 1, Quality { max_depth: 6, soft_shadows: true }),
        ("final   1280x720 1spp prof.6", 1280, 720, 1, Quality { max_depth: 6, soft_shadows: true }),
    ];
    // mostramos cuántos hilos hay, porque el render reparte las filas entre todos ellos
    println!("Hilos: {}", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1));
    for (name, w, h, spp, q) in modes {
        let _ = render_image(scene, HERO_YAW_DEG, cam, w, h, spp, q); // render de calentamiento que no medimos, para que cachés y memoria ya estén listos
        // medimos varias corridas seguidas y sacamos el promedio para que el resultado sea más estable
        let runs = 3;
        let t = Instant::now();
        for _ in 0..runs {
            let _ = render_image(scene, HERO_YAW_DEG, cam, w, h, spp, q);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / runs as f64;
        // los cuadros por segundo salen de dividir 1000 entre los milisegundos por cuadro
        println!("{name}: {ms:8.1} ms/cuadro ({:.1} fps)", 1000.0 / ms);
    }
}
