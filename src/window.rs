//! Ventana interactiva usando directamente la API de Windows (user32/gdi32) mediante FFI.
//! No es una librería externa: son llamadas al sistema operativo declaradas a mano.
//!
//! Controles:
//!   ← / → , A / D ........ rotar el diorama
//!   ↑ / ↓ ................ inclinar la cámara
//!   W / S , rueda del mouse  acercar / alejar (distancia real de la cámara)
//!   arrastrar con el mouse  rotar e inclinar
//!   1 / 2 / 3 ............ calidad: rápida / balanceada / alta
//!   R .................... volver a la vista inicial
//!   P .................... guardar captura PNG en capturas/
//!   Esc .................. salir

use crate::{hero_camera, EXPOSURE, HERO_YAW_DEG};
use las_noches::image_io::{save_png, Rgb8};
use las_noches::math::Vec3;
use las_noches::renderer::{render_accumulate, tonemap, Quality, Tracer, View};
use las_noches::scene::Scene;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::time::Instant;

type Hwnd = *mut c_void;
type Hdc = *mut c_void;

#[repr(C)]
struct WndClassW {
    style: u32,
    wnd_proc: extern "system" fn(Hwnd, u32, usize, isize) -> isize,
    cls_extra: i32,
    wnd_extra: i32,
    instance: *mut c_void,
    icon: *mut c_void,
    cursor: *mut c_void,
    background: *mut c_void,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
    private: u32,
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

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

#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    colors: [u32; 1],
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(wc: *const WndClassW) -> u16;
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
    fn DefWindowProcW(h: Hwnd, msg: u32, w: usize, l: isize) -> isize;
    fn PeekMessageW(msg: *mut Msg, h: Hwnd, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn PostQuitMessage(code: i32);
    fn GetDC(h: Hwnd) -> Hdc;
    fn ReleaseDC(h: Hwnd, dc: Hdc) -> i32;
    fn GetClientRect(h: Hwnd, r: *mut Rect) -> i32;
    fn AdjustWindowRect(r: *mut Rect, style: u32, menu: i32) -> i32;
    fn GetAsyncKeyState(key: i32) -> i16;
    fn GetForegroundWindow() -> Hwnd;
    fn GetCursorPos(p: *mut Point) -> i32;
    fn SetWindowTextW(h: Hwnd, text: *const u16) -> i32;
    fn LoadCursorW(inst: *mut c_void, name: *const u16) -> *mut c_void;
}

#[link(name = "gdi32")]
unsafe extern "system" {
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
    fn SetStretchBltMode(dc: Hdc, mode: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WM_DESTROY: u32 = 0x0002;
const WM_MOUSEWHEEL: u32 = 0x020A;
const PM_REMOVE: u32 = 1;
const WM_QUIT: u32 = 0x0012;
const SRCCOPY: u32 = 0x00CC_0020;
const HALFTONE: i32 = 4;

static WHEEL: AtomicI32 = AtomicI32::new(0);
static QUIT: AtomicBool = AtomicBool::new(false);

extern "system" fn wnd_proc(h: Hwnd, msg: u32, w: usize, l: isize) -> isize {
    match msg {
        WM_DESTROY => {
            QUIT.store(true, Ordering::Relaxed);
            unsafe { PostQuitMessage(0) };
            0
        }
        WM_MOUSEWHEEL => {
            let delta = ((w >> 16) & 0xFFFF) as u16 as i16;
            WHEEL.fetch_add(delta as i32, Ordering::Relaxed);
            0
        }
        _ => unsafe { DefWindowProcW(h, msg, w, l) },
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn key(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

/// Presets de calidad: (escala de resolución en movimiento, calidad en movimiento, calidad en reposo).
fn preset(level: u32) -> (usize, Quality, Quality) {
    match level {
        1 => (4, Quality { max_depth: 2, soft_shadows: false }, Quality { max_depth: 3, soft_shadows: false }),
        3 => (2, Quality { max_depth: 4, soft_shadows: true }, Quality { max_depth: 8, soft_shadows: true }),
        _ => (3, Quality { max_depth: 3, soft_shadows: false }, Quality { max_depth: 6, soft_shadows: true }),
    }
}

/// `autotest`: recorre un guion de entrada (rotar, acercar, alejar), guarda una captura y sale.
/// Sirve para verificar la ventana sin teclado.
pub fn run(scene: &Scene, autotest: bool) {
    const W: usize = 1280;
    const H: usize = 720;
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class = wide("LasNochesRaytracer");
        let wc = WndClassW {
            style: 0,
            wnd_proc,
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: std::ptr::null_mut(),
            cursor: LoadCursorW(std::ptr::null_mut(), 32512 as *const u16), // IDC_ARROW
            background: std::ptr::null_mut(),
            menu_name: std::ptr::null(),
            class_name: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let mut r = Rect { left: 0, top: 0, right: W as i32, bottom: H as i32 };
        AdjustWindowRect(&mut r, WS_OVERLAPPEDWINDOW, 0);
        let title = wide("Las Noches bajo la Luna Eterna — Raytracer");
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

        let mut yaw = HERO_YAW_DEG.to_radians();
        let mut cam = hero_camera();
        let mut level = 2u32;
        let mut accum = vec![Vec3::ZERO; W * H];
        let mut samples = 0u32; // muestras acumuladas en reposo (resolución completa)
        let mut bgra = vec![0u8; W * H * 4];
        let mut rgb = vec![0u8; W * H * 3];
        let mut last = Instant::now();
        let mut last_mouse: Option<Point> = None;
        let mut p_was_down = false;
        let mut status_timer = Instant::now();
        let mut last_frame_ms = 0.0f32;
        const MAX_SAMPLES: u32 = 64;
        let mut frame = 0u32;

        loop {
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
            let dt = last.elapsed().as_secs_f32().min(0.1);
            last = Instant::now();

            // --- Entrada ---
            let focused = GetForegroundWindow() == hwnd;
            let mut moved = false;
            if focused {
                let rot_speed = 1.4 * dt;
                let (left, right) = (key(0x25) || key(b'A' as i32), key(0x27) || key(b'D' as i32));
                if left {
                    yaw -= rot_speed;
                    moved = true;
                }
                if right {
                    yaw += rot_speed;
                    moved = true;
                }
                if key(0x26) {
                    cam.pitch += 0.8 * dt;
                    moved = true;
                }
                if key(0x28) {
                    cam.pitch -= 0.8 * dt;
                    moved = true;
                }
                if key(b'W' as i32) || key(0xBB) || key(0x6B) {
                    cam.distance *= 1.0 - 0.9 * dt;
                    moved = true;
                }
                if key(b'S' as i32) || key(0xBD) || key(0x6D) {
                    cam.distance *= 1.0 + 0.9 * dt;
                    moved = true;
                }
                if key(b'R' as i32) {
                    yaw = HERO_YAW_DEG.to_radians();
                    cam = hero_camera();
                    moved = true;
                }
                for (k, lv) in [(b'1', 1u32), (b'2', 2), (b'3', 3)] {
                    if key(k as i32) && level != lv {
                        level = lv;
                        moved = true;
                    }
                }
                if key(0x1B) {
                    break;
                }
                // Arrastre con el botón izquierdo.
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
                    last_mouse = None;
                }
            }
            if autotest {
                // Guion: 25 cuadros rotando, 25 acercando, 25 alejando, luego reposo.
                match frame {
                    0..25 => yaw += 0.03,
                    25..50 => cam.distance *= 0.98,
                    50..75 => cam.distance *= 1.01,
                    _ => {}
                }
                moved = frame < 75;
                frame += 1;
                if frame == 25 || frame == 50 || frame == 75 {
                    println!("autotest cuadro {frame}: rot {:.1}°, dist {:.2}, {last_frame_ms:.1} ms/cuadro", yaw.to_degrees(), cam.distance);
                }
                if samples >= 8 {
                    let mut img = Rgb8::new(W, H);
                    img.data.copy_from_slice(&rgb);
                    let _ = save_png(std::path::Path::new("capturas/autotest.png"), &img);
                    println!("autotest: 8 muestras acumuladas, último cuadro {last_frame_ms:.1} ms; captura en capturas/autotest.png");
                    break;
                }
            }
            let wheel = WHEEL.swap(0, Ordering::Relaxed);
            if wheel != 0 {
                cam.distance *= (1.0f32 - 0.10).powf(wheel as f32 / 120.0);
                moved = true;
            }
            cam.clamp();

            // --- Render ---
            let (scale, q_move, q_still) = preset(level);
            let view = View::new(scene, yaw, cam);
            let t0 = Instant::now();
            if moved {
                // En movimiento: baja resolución, pocos rebotes, se reinicia la acumulación.
                samples = 0;
                let (w, h) = (W / scale, H / scale);
                let tracer = Tracer { scene, view: &view, q: q_move };
                let mut small = vec![Vec3::ZERO; w * h];
                render_accumulate(&tracer, w, h, &mut small, 1, 0);
                let mut small_rgb = vec![0u8; w * h * 3];
                tonemap(&small, 1, w, h, EXPOSURE, &mut small_rgb);
                present(hwnd, &small_rgb, w, h, &mut bgra);
            } else if samples < MAX_SAMPLES {
                // En reposo: resolución completa, 1 muestra más por cuadro (refinamiento progresivo).
                if samples == 0 {
                    accum.iter_mut().for_each(|c| *c = Vec3::ZERO);
                }
                let tracer = Tracer { scene, view: &view, q: q_still };
                render_accumulate(&tracer, W, H, &mut accum, 1, samples);
                samples += 1;
                tonemap(&accum, samples, W, H, EXPOSURE, &mut rgb);
                present(hwnd, &rgb, W, H, &mut bgra);
            } else {
                std::thread::sleep(std::time::Duration::from_millis(16));
            }
            if moved || samples <= MAX_SAMPLES {
                let ms = t0.elapsed().as_secs_f32() * 1000.0;
                if ms > 0.5 {
                    last_frame_ms = ms;
                }
            }

            // Captura PNG (flanco de subida de la tecla P).
            let p_down = focused && key(b'P' as i32);
            if p_down && !p_was_down && samples > 0 {
                let mut img = Rgb8::new(W, H);
                img.data.copy_from_slice(&rgb);
                let name = format!("capturas/captura_{}.png", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
                match save_png(std::path::Path::new(&name), &img) {
                    Ok(()) => println!("Captura guardada: {name}"),
                    Err(e) => eprintln!("No se pudo guardar la captura: {e}"),
                }
            }
            p_was_down = p_down;

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

/// Copia un buffer RGB8 a la ventana, escalándolo al tamaño del área cliente.
fn present(hwnd: Hwnd, rgb: &[u8], w: usize, h: usize, bgra: &mut Vec<u8>) {
    bgra.resize(w * h * 4, 0);
    for i in 0..w * h {
        bgra[i * 4] = rgb[i * 3 + 2];
        bgra[i * 4 + 1] = rgb[i * 3 + 1];
        bgra[i * 4 + 2] = rgb[i * 3];
        bgra[i * 4 + 3] = 0;
    }
    let info = BitmapInfo {
        header: BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: w as i32,
            height: -(h as i32), // negativo = filas de arriba hacia abajo
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
        let mut r = Rect { left: 0, top: 0, right: 0, bottom: 0 };
        GetClientRect(hwnd, &mut r);
        let dc = GetDC(hwnd);
        SetStretchBltMode(dc, HALFTONE);
        StretchDIBits(dc, 0, 0, r.right, r.bottom, 0, 0, w as i32, h as i32, bgra.as_ptr() as *const c_void, &info, 0, SRCCOPY);
        ReleaseDC(hwnd, dc);
    }
}
