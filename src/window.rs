//! ventana interactiva hecha directamente con la api de windows (user32 y gdi32) por medio de ffi.
//! no usamos ninguna librería externa: las funciones del sistema operativo las declaramos a mano
//! más abajo con `extern "system"` y las llamamos dentro de bloques `unsafe`.
//!
//! la idea general es esta: abrimos una ventana, en cada vuelta del ciclo leemos el teclado y el
//! mouse, movemos la cámara, renderizamos la escena con nuestro raytracer y copiamos los píxeles
//! a la ventana con `StretchDIBits`. para que se sienta fluido usamos calidad adaptativa: mientras
//! la cámara se mueve renderizamos a baja resolución, y cuando se queda quieta vamos sumando
//! muestras a resolución completa para que la imagen se vaya limpiando poco a poco.
//!
//! controles:
//!   flecha izquierda y flecha derecha, o las teclas a y d: rotar el diorama
//!   flecha arriba y flecha abajo: inclinar la cámara
//!   teclas w y s, teclas más y menos, o la rueda del mouse: acercar y alejar (distancia real de la cámara)
//!   arrastrar con el botón izquierdo del mouse: rotar e inclinar al mismo tiempo
//!   teclas 1, 2 y 3: calidad rápida, balanceada o alta
//!   tecla r: volver a la vista inicial
//!   tecla p: guardar una captura png en la carpeta capturas
//!   tecla esc: salir

use crate::{hero_camera, EXPOSURE, HERO_YAW_DEG};
use las_noches::image_io::{save_png, Rgb8};
use las_noches::math::Vec3;
use las_noches::renderer::{render_accumulate, tonemap, Quality, Tracer, View};
use las_noches::scene::Scene;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Instant;

/// identificador de una ventana en windows (el `HWND` de c). para nosotros es solo un puntero opaco.
type Hwnd = *mut c_void;
/// contexto de dispositivo (el `HDC` de c), es lo que gdi necesita para poder dibujar en la ventana.
type Hdc = *mut c_void;

// todas las estructuras que le pasamos a windows llevan `#[repr(C)]`. sin eso rust podría
// reordenar los campos o meter relleno distinto, y windows leería basura. con `repr(C)` los
// campos quedan en memoria exactamente en el orden y alineación que espera la api escrita en c.

/// copia de la estructura `WNDCLASSW`: describe una "clase" de ventana (su procedimiento de
/// mensajes, el cursor, el nombre de la clase, etc). hay que registrarla antes de crear la ventana.
#[repr(C)]
struct WndClassW {
    style: u32,
    // puntero a la función que windows va a llamar cada vez que llegue un mensaje a la ventana
    wnd_proc: extern "system" fn(Hwnd, u32, usize, isize) -> isize,
    cls_extra: i32,
    wnd_extra: i32,
    instance: *mut c_void,
    icon: *mut c_void,
    cursor: *mut c_void,
    background: *mut c_void,
    menu_name: *const u16,
    // nombre de la clase en utf16 terminado en cero, así es como windows maneja los textos
    class_name: *const u16,
}

/// copia de `POINT`: un punto en píxeles, la usamos para la posición del cursor.
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

/// copia de `MSG`: un mensaje de la cola de la ventana (tecla, rueda, cerrar, etc).
#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
    // campo reservado que windows tiene al final de `MSG`, lo ponemos para que el tamaño coincida
    private: u32,
}

/// copia de `RECT`: un rectángulo dado por sus cuatro bordes en píxeles.
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// copia de `BITMAPINFOHEADER`: le dice a gdi cómo están guardados nuestros píxeles
/// (ancho, alto, bits por píxel, si hay compresión).
#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_ppm: i32,
    y_ppm: i32,
    clr_used: u32,
    clr_important: u32,
}

/// copia de `BITMAPINFO`: el encabezado más una paleta. como usamos 32 bits por píxel no
/// necesitamos paleta, pero la estructura en c trae una entrada y la dejamos para respetar el tamaño.
#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [u32; 1],
}

// funciones de user32.dll: todo lo de ventanas, mensajes, teclado y mouse.
// `extern "system"` usa la convención de llamada que usa la api de windows.
#[link(name = "user32")]
unsafe extern "system" {
    // registra la clase de ventana, devuelve un átomo distinto de cero si salió bien
    fn RegisterClassW(wc: *const WndClassW) -> u16;
    // crea la ventana real con su estilo, posición, tamaño y título
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Hwnd,
        menu: *mut c_void,
        instance: *mut c_void,
        param: *mut c_void,
    ) -> Hwnd;
    // comportamiento por defecto para los mensajes que nosotros no manejamos
    fn DefWindowProcW(h: Hwnd, msg: u32, w: usize, l: isize) -> isize;
    // revisa la cola de mensajes sin bloquear, así el ciclo de render nunca se queda esperando
    fn PeekMessageW(msg: *mut Msg, h: Hwnd, min: u32, max: u32, remove: u32) -> i32;
    // convierte mensajes de teclas virtuales en mensajes de caracteres
    fn TranslateMessage(msg: *const Msg) -> i32;
    // manda el mensaje a nuestro `wnd_proc`
    fn DispatchMessageW(msg: *const Msg) -> isize;
    // pone un `WM_QUIT` en la cola para avisar que el programa debe terminar
    fn PostQuitMessage(code: i32);
    // pide y devuelve el contexto de dispositivo de la ventana para dibujar
    fn GetDC(h: Hwnd) -> Hdc;
    fn ReleaseDC(h: Hwnd, dc: Hdc) -> i32;
    // tamaño del área cliente, o sea la parte de adentro sin bordes ni barra de título
    fn GetClientRect(h: Hwnd, r: *mut Rect) -> i32;
    // agranda un rectángulo para que, sumando bordes y título, el área cliente quede del tamaño pedido
    fn AdjustWindowRect(r: *mut Rect, style: u32, menu: i32) -> i32;
    // estado actual de una tecla en este instante, sin pasar por la cola de mensajes
    fn GetAsyncKeyState(key: i32) -> i16;
    // ventana que tiene el foco, para no reaccionar a teclas si estamos en otra aplicación
    fn GetForegroundWindow() -> Hwnd;
    // posición del cursor en coordenadas de pantalla
    fn GetCursorPos(p: *mut Point) -> i32;
    // cambia el texto de la barra de título, ahí mostramos el estado
    fn SetWindowTextW(h: Hwnd, text: *const u16) -> i32;
    // carga uno de los cursores estándar del sistema
    fn LoadCursorW(inst: *mut c_void, name: *const u16) -> *mut c_void;
}

// funciones de gdi32.dll: el dibujo de bitmaps en la ventana.
#[link(name = "gdi32")]
unsafe extern "system" {
    // copia un bitmap que está en nuestra memoria a la ventana, escalándolo si hace falta
    fn StretchDIBits(
        dc: Hdc,
        xd: i32,
        yd: i32,
        wd: i32,
        hd: i32,
        xs: i32,
        ys: i32,
        ws: i32,
        hs: i32,
        bits: *const c_void,
        info: *const BitmapInfo,
        usage: u32,
        rop: u32,
    ) -> i32;
    // elige cómo se escala el bitmap (con `HALFTONE` se promedian los píxeles y se ve más suave)
    fn SetStretchBltMode(dc: Hdc, mode: i32) -> i32;
}

// de kernel32.dll solo necesitamos el handle del propio ejecutable.
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

/// estilo de ventana normal: con título, borde redimensionable y botones de minimizar, maximizar y cerrar.
const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
/// hace que la ventana se muestre apenas se crea.
const WS_VISIBLE: u32 = 0x1000_0000;
/// mensaje que llega cuando la ventana se está destruyendo (por ejemplo al darle a la x).
const WM_DESTROY: u32 = 0x0002;
/// mensaje que llega cuando se gira la rueda del mouse.
const WM_MOUSEWHEEL: u32 = 0x020A;
/// le dice a `PeekMessageW` que saque el mensaje de la cola después de leerlo.
const PM_REMOVE: u32 = 1;
/// mensaje especial que pide terminar el ciclo de mensajes.
const WM_QUIT: u32 = 0x0012;
/// operación de raster que copia los píxeles tal cual, sin mezclarlos con lo que ya había.
const SRCCOPY: u32 = 0x00CC_0020;
/// modo de escalado que promedia píxeles, se ve mejor al agrandar la imagen chica.
const HALFTONE: i32 = 4;

// el `wnd_proc` es una función que llama windows y no recibe nuestro estado, por eso la rueda y
// la señal de salir se comunican con variables estáticas atómicas. todo corre en el mismo hilo,
// así que con `Ordering::Relaxed` alcanza.

/// suma de los movimientos de la rueda que llegaron desde la última vez que la leímos.
static WHEEL: AtomicI32 = AtomicI32::new(0);
/// se pone en verdadero cuando hay que cerrar la aplicación.
static QUIT: AtomicBool = AtomicBool::new(false);

/// procedimiento de ventana: windows lo llama con cada mensaje que le llega a nuestra ventana.
/// solo nos interesan cerrar y la rueda, todo lo demás se lo devolvemos a `DefWindowProcW`.
extern "system" fn wnd_proc(h: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_DESTROY => {
            // marcamos que hay que salir y metemos el `WM_QUIT` en la cola
            QUIT.store(true, Ordering::Relaxed);
            unsafe { PostQuitMessage(0) };
            0
        }
        WM_MOUSEWHEEL => {
            // el giro de la rueda viene en la palabra alta de `wparam` (los 16 bits de arriba).
            // lo pasamos primero a u16 y luego a i16 para recuperar el signo: positivo es girar
            // hacia adelante y negativo hacia atrás. cada "clic" de la rueda vale 120.
            let delta = ((w >> 16) & 0xFFFF) as u16 as i16;
            WHEEL.fetch_add(delta as i32, Ordering::Relaxed);
            0
        }
        _ => unsafe { DefWindowProcW(h, msg, w, l) },
    }
}

/// convierte un texto de rust a utf16 con un cero al final, que es el formato de las funciones `W` de windows.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// dice si la tecla virtual `vk` está presionada en este momento. el bit más alto (0x8000)
/// del resultado de `GetAsyncKeyState` es el que indica que la tecla está abajo.
fn key(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

/// presets de calidad. devuelve (cuánto dividimos la resolución en movimiento, calidad en
/// movimiento, calidad en reposo). `max_depth` es el máximo de rebotes del rayo y `soft_shadows`
/// activa las sombras suaves. el nivel 1 es el más rápido y el 3 el más bonito.
fn preset(level: u32) -> (usize, Quality, Quality) {
    match level {
        1 => (4, Quality { max_depth: 2, soft_shadows: false }, Quality { max_depth: 3, soft_shadows: false }),
        3 => (2, Quality { max_depth: 4, soft_shadows: true }, Quality { max_depth: 8, soft_shadows: true }),
        _ => (3, Quality { max_depth: 3, soft_shadows: false }, Quality { max_depth: 6, soft_shadows: true }),
    }
}

/// abre la ventana y corre el ciclo principal hasta que el usuario la cierre.
/// si `autotest` es verdadero se sigue un guion de entrada (rotar, acercar, alejar), se guarda
/// una captura y se sale solo. sirve para probar la ventana sin tocar el teclado.
pub fn run(scene: &Scene, autotest: bool) {
    // resolución interna completa del render
    const W: usize = 1280;
    const H: usize = 720;
    unsafe {
        // handle de nuestro propio ejecutable, windows lo pide para registrar la clase
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("LasNochesRaytracer");
        // llenamos la descripción de la clase de ventana con nuestro `wnd_proc`
        let wc = WndClassW {
            style: 0,
            wnd_proc,
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: std::ptr::null_mut(),
            cursor: LoadCursorW(std::ptr::null_mut(), 32512 as *const u16), // 32512 es `IDC_ARROW`, la flecha normal
            background: std::ptr::null_mut(),
            menu_name: std::ptr::null(),
            class_name: class.as_ptr(),
        };
        RegisterClassW(&wc);
        // queremos que el área cliente mida exactamente w por h, así que ajustamos el rectángulo
        // para incluir los bordes y la barra de título
        let mut r = Rect { left: 0, top: 0, right: W as i32, bottom: H as i32 };
        AdjustWindowRect(&mut r, WS_OVERLAPPEDWINDOW, 0);
        let title = wide("Las Noches bajo la Luna Eterna — Raytracer");
        // creamos la ventana en la posición 80, 60 de la pantalla con el tamaño ya ajustado
        let hwnd = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            80,
            60,
            r.right - r.left,
            r.bottom - r.top,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null_mut(),
        );
        if hwnd.is_null() {
            eprintln!("No se pudo crear la ventana.");
            return;
        }

        // estado de la cámara: el giro del diorama va aparte y la cámara guarda inclinación y distancia
        let mut yaw = HERO_YAW_DEG.to_radians();
        let mut cam = hero_camera();
        let mut level = 2u32;
        // buffer donde sumamos las muestras de color de cada píxel cuando la cámara está quieta
        let mut accum = vec![Vec3::ZERO; W * H];
        let mut samples = 0u32; // cuántas muestras llevamos acumuladas en reposo a resolución completa
        // buffer en el orden azul, verde, rojo, relleno que pide gdi
        let mut bgra = vec![0u8; W * H * 4];
        // última imagen completa en rgb de 8 bits, la usamos también para las capturas
        let mut rgb = vec![0u8; W * H * 3];
        let mut last = Instant::now();
        // posición anterior del mouse mientras se arrastra, `None` si no se está arrastrando
        let mut last_mouse: Option<Point> = None;
        // para detectar solo el momento en que se presiona p y no guardar una captura por cuadro
        let mut p_was_down = false;
        let mut status_timer = Instant::now();
        let mut last_frame_ms = 0.0f32;
        // al llegar a este número de muestras la imagen ya está limpia y dejamos de renderizar
        const MAX_SAMPLES: u32 = 64;
        // contador de cuadros, solo lo usa el modo autotest
        let mut frame = 0u32;

        loop {
            // ciclo de mensajes: vaciamos la cola sin bloquear. cada mensaje se traduce y se
            // despacha, y así windows termina llamando a nuestro `wnd_proc`
            let mut msg: Msg = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    QUIT.store(true, Ordering::Relaxed);
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if QUIT.load(Ordering::Relaxed) {
                break;
            }
            // tiempo desde el cuadro anterior en segundos, lo limitamos a 0.1 para que un cuadro
            // lento no haga saltar la cámara de golpe
            let dt = last.elapsed().as_secs_f32().min(0.1);
            last = Instant::now();

            // entrada: aquí leemos el teclado y el mouse
            // solo hacemos caso a las teclas si nuestra ventana es la que tiene el foco
            let focused = GetForegroundWindow() == hwnd;
            // si algo cambia la cámara marcamos `moved` para reiniciar la acumulación
            let mut moved = false;
            if focused {
                // velocidad de giro en radianes por segundo multiplicada por dt, así no depende de los fps
                let rot_speed = 1.4 * dt;
                // 0x25 es la flecha izquierda y 0x27 la flecha derecha; también sirven a y d
                let (left, right) = (key(0x25) || key(b'A' as i32), key(0x27) || key(b'D' as i32));
                if left {
                    yaw -= rot_speed;
                    moved = true;
                }
                if right {
                    yaw += rot_speed;
                    moved = true;
                }
                // 0x26 es la flecha arriba: subimos la inclinación
                if key(0x26) {
                    cam.pitch += 0.8 * dt;
                    moved = true;
                }
                // 0x28 es la flecha abajo: bajamos la inclinación
                if key(0x28) {
                    cam.pitch -= 0.8 * dt;
                    moved = true;
                }
                // acercar con w, con la tecla más (`0xBB`) o con el más del teclado numérico (`0x6B`).
                // multiplicamos la distancia en vez de restarle, así el zoom se siente igual de cerca y de lejos
                if key(b'W' as i32) || key(0xBB) || key(0x6B) {
                    cam.distance *= 1.0 - 0.9 * dt;
                    moved = true;
                }
                // alejar con s, con la tecla menos (`0xBD`) o con el menos del teclado numérico (`0x6D`)
                if key(b'S' as i32) || key(0xBD) || key(0x6D) {
                    cam.distance *= 1.0 + 0.9 * dt;
                    moved = true;
                }
                // r regresa todo a la vista héroe del inicio
                if key(b'R' as i32) {
                    yaw = HERO_YAW_DEG.to_radians();
                    cam = hero_camera();
                    moved = true;
                }
                // teclas 1, 2 y 3 para cambiar la calidad; si cambia hay que volver a empezar a acumular
                for (k, lv) in [(b'1', 1u32), (b'2', 2), (b'3', 3)] {
                    if key(k as i32) && level != lv {
                        level = lv;
                        moved = true;
                    }
                }
                // `0x1B` es esc: salimos del ciclo
                if key(0x1B) {
                    break;
                }
                // arrastre con el botón izquierdo (tecla virtual 0x01). comparamos la posición
                // del cursor con la del cuadro anterior: lo horizontal gira y lo vertical inclina
                if key(0x01) {
                    let mut p = Point { x: 0, y: 0 };
                    GetCursorPos(&mut p);
                    if let Some(lp) = &last_mouse {
                        let (dx, dy) = (p.x - lp.x, p.y - lp.y);
                        if dx != 0 || dy != 0 {
                            yaw += dx as f32 * 0.006;
                            cam.pitch += dy as f32 * 0.004;
                            moved = true;
                        }
                    }
                    last_mouse = Some(p);
                } else {
                    // al soltar el botón olvidamos la posición para que el próximo arrastre no salte
                    last_mouse = None;
                }
            }
            if autotest {
                // guion de prueba: 25 cuadros rotando, 25 acercando, 25 alejando y luego reposo
                match frame {
                    0..25 => yaw += 0.03,
                    25..50 => cam.distance *= 0.98,
                    50..75 => cam.distance *= 1.01,
                    _ => {}
                }
                moved = frame < 75;
                frame += 1;
                // al final de cada tramo imprimimos cómo va la cámara y cuánto tarda un cuadro
                if frame == 25 || frame == 50 || frame == 75 {
                    println!("autotest cuadro {frame}: rot {:.1}°, dist {:.2}, {last_frame_ms:.1} ms/cuadro", yaw.to_degrees(), cam.distance);
                }
                // con 8 muestras en reposo ya se ve la imagen refinada: guardamos la captura y salimos
                if samples >= 8 {
                    let mut img = Rgb8::new(W, H);
                    img.data.copy_from_slice(&rgb);
                    let _ = save_png(std::path::Path::new("capturas/autotest.png"), &img);
                    println!("autotest: 8 muestras acumuladas, último cuadro {last_frame_ms:.1} ms; captura en capturas/autotest.png");
                    break;
                }
            }
            // leemos lo que acumuló la rueda y lo dejamos en cero de una vez con `swap`.
            // cada 120 unidades es un clic y cada clic acerca un 10 por ciento (o aleja si es negativo)
            let wheel = WHEEL.swap(0, Ordering::Relaxed);
            if wheel != 0 {
                cam.distance *= (1.0f32 - 0.10).powf(wheel as f32 / 120.0);
                moved = true;
            }
            // limitamos distancia e inclinación para no meter la cámara bajo el suelo ni demasiado lejos
            cam.clamp();

            // render con calidad adaptativa
            let (scale, q_move, q_still) = preset(level);
            // la vista junta la escena girada con la cámara actual
            let view = View::new(scene, yaw, cam);
            let t0 = Instant::now();
            if moved {
                // en movimiento: baja resolución y pocos rebotes para que responda rápido.
                // ponemos `samples` en cero porque lo acumulado ya no sirve con la cámara nueva
                samples = 0;
                let (w, h) = (W / scale, H / scale);
                let tracer = Tracer { scene, view: &view, q: q_move };
                let mut small = vec![Vec3::ZERO; w * h];
                // una sola muestra por píxel en el buffer chico
                render_accumulate(&tracer, w, h, &mut small, 1, 0);
                let mut small_rgb = vec![0u8; w * h * 3];
                // tone mapping: pasamos de color en rango alto a bytes de 0 a 255 aplicando la exposición
                tonemap(&small, 1, w, h, EXPOSURE, &mut small_rgb);
                // `present` estira la imagen chica al tamaño de la ventana
                present(hwnd, &small_rgb, w, h, &mut bgra);
            } else if samples < MAX_SAMPLES {
                // en reposo: resolución completa y una muestra más por cuadro (refinamiento progresivo)
                if samples == 0 {
                    // primer cuadro quieto: borramos la suma anterior para empezar desde cero
                    accum.iter_mut().for_each(|c| *c = Vec3::ZERO);
                }
                let tracer = Tracer { scene, view: &view, q: q_still };
                // sumamos una muestra nueva a cada píxel; pasamos `samples` como índice base para
                // que cada muestra use una variación distinta y no repita la misma
                render_accumulate(&tracer, W, H, &mut accum, 1, samples);
                samples += 1;
                // tone mapping de la suma dividida entre el número de muestras, o sea el promedio
                tonemap(&accum, samples, W, H, EXPOSURE, &mut rgb);
                present(hwnd, &rgb, W, H, &mut bgra);
            } else {
                // ya llegamos al máximo de muestras: no hay nada nuevo que dibujar, dormimos un
                // poco para no gastar procesador
                std::thread::sleep(std::time::Duration::from_millis(16));
            }
            // guardamos cuánto tardó el cuadro para mostrarlo en el título; ignoramos tiempos
            // casi nulos que no dicen nada
            if moved || samples <= MAX_SAMPLES {
                let ms = t0.elapsed().as_secs_f32() * 1000.0;
                if ms > 0.5 {
                    last_frame_ms = ms;
                }
            }

            // captura png: solo en el flanco de subida de la tecla p, es decir, en el cuadro en
            // que pasa de suelta a presionada. también pedimos al menos una muestra en `rgb`
            let p_down = focused && key(b'P' as i32);
            if p_down && !p_was_down && samples > 0 {
                let mut img = Rgb8::new(W, H);
                img.data.copy_from_slice(&rgb);
                // el nombre lleva los segundos desde 1970 para que nunca se pisen dos capturas
                let name = format!("capturas/captura_{}.png", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
                match save_png(std::path::Path::new(&name), &img) {
                    Ok(()) => println!("Captura guardada: {name}"),
                    Err(e) => eprintln!("No se pudo guardar la captura: {e}"),
                }
            }
            p_was_down = p_down;

            // cada 250 ms actualizamos la barra de título con el estado de la cámara y el rendimiento
            if status_timer.elapsed().as_millis() > 250 {
                status_timer = Instant::now();
                let t = wide(&format!(
                    "Las Noches — rot {:.0}°  incl {:.0}°  dist {:.1}  calidad {}  muestras {}  {:.0} ms/cuadro   [←→ rotar · ↑↓ inclinar · W/S/rueda zoom · 1-3 calidad · R · P · Esc]",
                    yaw.to_degrees().rem_euclid(360.0),
                    cam.pitch.to_degrees(),
                    cam.distance,
                    level,
                    samples,
                    last_frame_ms
                ));
                SetWindowTextW(hwnd, t.as_ptr());
            }
        }
    }
}

/// copia un buffer rgb de 8 bits a la ventana, escalándolo al tamaño actual del área cliente.
/// `bgra` es un buffer de trabajo que reutilizamos entre cuadros para no pedir memoria cada vez.
fn present(hwnd: Hwnd, rgb: &[u8], w: usize, h: usize, bgra: &mut Vec<u8>) {
    bgra.resize(w * h * 4, 0);
    // gdi espera los píxeles de 32 bits en orden azul, verde, rojo y un byte de relleno,
    // así que volteamos el orden de los canales
    for i in 0..w * h {
        bgra[i * 4] = rgb[i * 3 + 2];
        bgra[i * 4 + 1] = rgb[i * 3 + 1];
        bgra[i * 4 + 2] = rgb[i * 3];
        bgra[i * 4 + 3] = 0;
    }
    // describimos el bitmap: 32 bits por píxel, sin compresión (0 es `BI_RGB`)
    let info = BitmapInfo {
        header: BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: w as i32,
            height: -(h as i32), // alto negativo significa que las filas van de arriba hacia abajo, como en nuestro buffer
            planes: 1,
            bit_count: 32,
            compression: 0,
            size_image: 0,
            x_ppm: 0,
            y_ppm: 0,
            clr_used: 0,
            clr_important: 0,
        },
        colors: [0],
    };
    unsafe {
        // medimos el área cliente por si el usuario cambió el tamaño de la ventana
        let mut r = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(hwnd, &mut r);
        let dc = GetDC(hwnd);
        SetStretchBltMode(dc, HALFTONE);
        // copiamos todo el bitmap de w por h al rectángulo completo de la ventana. el 0 antes de
        // `SRCCOPY` es `DIB_RGB_COLORS`, que indica colores directos y no índices de paleta
        StretchDIBits(dc, 0, 0, r.right, r.bottom, 0, 0, w as i32, h as i32, bgra.as_ptr() as *const c_void, &info, 0, SRCCOPY);
        // devolvemos el contexto de dispositivo, windows tiene pocos y hay que liberarlos
        ReleaseDC(hwnd, dc);
    }
}
