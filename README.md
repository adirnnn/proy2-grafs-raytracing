# Las Noches bajo la Luna Eterna

**Proyecto 2 — Gráficas por Computadora: Diorama con Raytracing**

**Hueco Mundo** (*Bleach*): un desierto blanco y sin fin bajo una noche eterna. En el centro se alza **Las Noches**, la fortaleza de Aizen:

- un tambor colosal con una cúpula baja;
- seis torres cilíndricas con base acampanada;
- ventanas encendidas;
- una calzada de **obsidiana pulida** que refleja la puerta iluminada como si fuera agua.

En primer plano, **Zangetsu**, a su escala real, está clavada en la calzada. Detrás del espectador se abre una **Garganta** roja que solo se ve en los reflejos.

Como en una maqueta de arquitectura, un **corte** en la parte trasera de la fortaleza deja ver lo que esconde: el **cielo diurno falso** de su interior (así es en la obra original) y el salón del trono de mármol pulido. Al girar el diorama, de noche afuera aparece el día adentro.

Todo está hecho con un raytracer escrito desde cero en **Rust, usando únicamente la biblioteca estándar** (0 dependencias). Las texturas y el skybox también son originales y los genera el propio proyecto.

![Vista héroe del diorama](docs/img/hero.png)

## Video

[![Ver el video del diorama](docs/img/video_thumb.png)](docs/video/las_noches.mp4)

> Clic en la imagen para abrir el video (`docs/video/las_noches.mp4`, 60 s, 1280×720, 30 fps).

![Vista previa animada](docs/img/preview.gif)

**Recorrido del video:**

| Segundos | Qué muestra |
|---|---|
| 0–5 | Al ras del suelo sobre la calzada: Las Noches y la puerta reflejadas en la obsidiana |
| 5–9 | La cámara sube: la fortaleza completa entre la niebla |
| 9–24 | Rotación completa del diorama (360°) |
| 24–35 | El corte: el cielo falso del interior y su reflejo en el mármol pulido |
| 35–44 | Un tronco de cuarzo refracta el anillo de luz de la cúpula |
| 44–52 | Zangetsu a escala real, con su reflejo y Las Noches detrás |
| 52–57 | Alejamiento: el desierto infinito bajo la niebla y el skybox |
| 57–60 | Regreso a la vista héroe |

El video se genera con `--video` a 1280×720, 6 muestras por píxel y 30 fps. Es determinista.

---

## Cómo ejecutar

**Requisitos:** [Rust](https://www.rust-lang.org/) (probado con 1.97, edición 2024) en **Windows 10/11**. No hay nada más que instalar: el proyecto no tiene dependencias (`cargo tree` muestra solo `las_noches`).

```bash
cargo run --release
```

Eso abre la ventana interactiva. Otros modos:

| Comando | Qué hace |
|---|---|
| `cargo run --release` | Ventana interactiva (rotación y zoom en vivo) |
| `cargo run --release -- --render salida.png --w 1920 --h 1080 --spp 32` | Render final a PNG (opcional: `--yaw`, `--pitch`, `--dist`, `--target x,y,z`, `--depth`) |
| `cargo run --release -- --video cuadros/ --spp 6` | Cuadros BMP del recorrido del video (reanudable, con `--from`/`--to`) |
| `cargo run --release -- --diag carpeta/` | Escenas de diagnóstico (refracción, reflexión, UV, skybox) |
| `cargo run --release -- --bench` | Mide el tiempo por cuadro de cada modo de calidad |
| `cargo run --release --bin gen_textures` | Regenera todas las texturas y el skybox en `assets/` |
| `cargo test --release` | Pruebas numéricas de óptica, UV, skybox y límites de la cámara |

> La ventana usa la API de Windows (`user32`/`gdi32`) declarada a mano por FFI. En otros sistemas operativos el render a PNG (`--render`) y el video funcionan igual.

## Controles

| Tecla | Acción |
|---|---|
| **← / →** o **A / D** | Rotar el diorama sobre su eje vertical (el centro de Las Noches) |
| **↑ / ↓** | Inclinar la cámara |
| **W / S**, **+ / −** o **rueda del mouse** | Acercar / alejar (cambia la **distancia real** de la cámara, no el FOV) |
| **Arrastrar con clic izquierdo** | Rotar e inclinar |
| **1 / 2 / 3** | Calidad rápida / balanceada / alta |
| **R** | Volver a la vista inicial (al ras de la calzada) |
| **P** | Guardar captura PNG en `capturas/` |
| **Esc** | Salir |

**Límites de la cámara:**

- La distancia va de 46 a 140, y la cámara nunca baja de la altura de los ojos de una persona.
- `cargo test` verifica que todo lo que sobresale del suelo queda dentro de la distancia mínima, así que el zoom no atraviesa la geometría.

**Refinamiento:** mientras se mueve, la imagen se calcula a baja resolución. Al soltar, se refina progresivamente a 1280×720 acumulando muestras, y se reinicia con cualquier cambio. El título de la ventana muestra rotación, inclinación, distancia, muestras y ms/cuadro.

**La rotación es del diorama, no una órbita de la cámara.** Los rayos se transforman al espacio del diorama, mientras que el cielo, la luna y la Garganta se quedan fijos en el mundo. Por eso la luz recorre la escena al girarla, como una maqueta sobre una base giratoria.

---

## Rúbrica y evidencia

> Nota: los puntos listados en la rúbrica suman **130**, mientras que la nota máxima indicada es **100**. Este proyecto cubre todos los criterios; la forma de escalar la nota la define el catedrático.

| Criterio | Puntos | Implementación | Evidencia |
|---|---:|---|---|
| Complejidad de la escena | 30 | `src/scene.rs`: **22,971 cubos** texturizados. Tambor y cúpula de cuarto de bloque con corte; 6 torres acampanadas con ventanas; salón del trono; dunas; calzada; 13 árboles de cuarzo; monolitos; lápidas; Zangetsu | Imágenes de abajo y video |
| Atractivo visual | 20 | Escena nocturna con niebla y luz de fuentes reales (puerta, cielo falso, luna). Composición basada en la iconografía de Las Noches; sin bordes visibles | `docs/img/hero.png`, `docs/img/angulos.png` |
| Rotación del diorama y zoom | 20 | `src/window.rs` (entrada y refinamiento), `src/renderer.rs::View` (rotación alrededor del pivote), `src/camera.rs` (zoom por distancia, límites) | Ventana interactiva; `docs/img/zoom.png`; video: 9–24 s rotación de 360°, 52–57 s alejamiento |
| 5 materiales (textura + albedo, especular, transparencia, reflectividad) | 25 | `src/material.rs`, texturas en `assets/textures/` | [Tabla de materiales](#materiales) |
| Refracción con sentido | 10 | Árboles muertos de **cuarzo** (IOR 1.54) con Snell, Fresnel y reflexión interna total | `docs/img/mat_cuarzo.png`, `docs/img/diag_frente.png`, video 35–44 s |
| Reflexión | 5 | **Obsidiana** de la calzada y los monolitos (0.65), **mármol pulido** del salón (0.35) y **acero** de Zangetsu (0.80) | `docs/img/hero.png`, `docs/img/mat_marmol.png`, `docs/img/zangetsu.png`, video 0–5, 24–35 y 44–52 s |
| Skybox | 20 | Cubemap de 6 caras BMP (`assets/skybox/`), muestreado por **todos** los rayos que no chocan (primarios, reflejados y refractados). La niebla toma su color del horizonte del mismo skybox | Luna, nubes y estrellas en `docs/img/hero.png`; la Garganta solo aparece reflejada |

## Materiales

Los valores vienen directamente de `src/material.rs`. Significado de cada parámetro:

- **albedo:** fracción de luz difusa que devuelve la superficie; se multiplica por el color de la textura.
- **especular / brillo:** intensidad y exponente del brillo de Phong.
- **transparencia:** fracción de luz transmitida (rayo refractado).
- **reflectividad:** fracción de luz reflejada como espejo.
- **IOR:** índice de refracción.

La parte difusa pesa `1 − reflectividad − transparencia`. En materiales transparentes, Fresnel (Schlick) pasa parte de la transparencia a reflexión en ángulos rasantes, así que la suma nunca supera 1.

| # | Material | Textura | Albedo | Especular (int., exp.) | Transparencia | Reflectividad | IOR | Dónde está | Evidencia |
|---|---|---|---:|---|---:|---:|---:|---|---|
| 1 | Arena de Hueco Mundo | [`sand.bmp`](assets/textures/sand.bmp) | 0.66 | 0.04, 8 | 0 | 0 | — | Desierto y dunas | [mat_arena](docs/img/mat_arena.png) |
| 2 | Concreto de Las Noches | [`stone.bmp`](assets/textures/stone.bmp) | 0.72 | 0.18, 30 | 0 | 0.04 | — | Tambor, cúpula, torres, puerta, lápidas | [mat_concreto](docs/img/mat_concreto.png) |
| 3 | Cuarzo | [`quartz.bmp`](assets/textures/quartz.bmp) | 0.32 | 0.8, 180 | 0.70 | 0.06 | 1.54 | Árboles muertos | [mat_cuarzo](docs/img/mat_cuarzo.png) |
| 4 | Obsidiana | [`obsidian.bmp`](assets/textures/obsidian.bmp) | 0.15 | 0.9, 300 | 0 | 0.65 | — | Calzada, monolitos, lomo de la espada | [hero](docs/img/hero.png) |
| 5 | Mármol pulido | [`marble.bmp`](assets/textures/marble.bmp) | 0.62 | 0.7, 160 | 0 | 0.35 | — | Piso del salón del trono | [mat_marmol](docs/img/mat_marmol.png) |
| 6 | Acero de Zangetsu | [`steel.bmp`](assets/textures/steel.bmp) | 0.22 | 1.0, 500 | 0 | 0.80 | — | Hoja de la espada | [zangetsu](docs/img/zangetsu.png) |
| + | Cielo falso (emisivo 1.0) | [`fake_sky.bmp`](assets/textures/fake_sky.bmp) | 0.35 | — | 0 | 0 | — | Caras interiores del tambor y la cúpula | mat_marmol |
| + | Ventanas encendidas (emisivo 2.2) | [`light.bmp`](assets/textures/light.bmp) | 0.20 | — | 0 | 0 | — | Ventanas, banda del tambor, vano de la puerta | hero |
| + | Vendaje | [`hilt.bmp`](assets/textures/hilt.bmp) | 0.80 | 0.05, 10 | 0 | 0 | — | Mango de Zangetsu | zangetsu |

![Texturas](docs/img/texturas.png)

## Arquitectura

```
src/
  main.rs         modos: ventana, --render, --video, --diag, --bench
  window.rs       ventana Win32 por FFI, entrada, calidad adaptativa, refinamiento progresivo
  video.rs        recorrido de cámara por claves (interpolación suave), cuadros deterministas
  diag.rs         escenas de diagnóstico
  lib.rs          módulos del raytracer:
  math.rs         Vec3, Mat3 (rotaciones), RNG determinista
  camera.rs       cámara orbital con zoom por distancia y límites
  geometry.rs     cubo (slabs, normal y UV por cara), esfera, objetos rotados
  bvh.rs          BVH sobre cajas envolventes (división por mediana)
  material.rs     parámetros de los materiales
  texture.rs      texturas (sRGB→lineal, bilineal) y skybox cubemap
  scene.rs        construcción del diorama, luces, niebla
  renderer.rs     trazado recursivo, sombreado, Fresnel, niebla, tone mapping
  image_io.rs     BMP (lectura/escritura) y PNG (escritura) sin librerías
  bin/gen_textures.rs   generador procedural de texturas y skybox
tests/optica.rs   pruebas de Snell, reflexión interna total, UV, skybox y límites
```

### Cómo se construye la escena con cubos

Escala: 1 unidad ≈ 4 m. La fortaleza mide ~85 m de diámetro, las torres ~115 m y Zangetsu ~2 m.

- **Desierto sin bordes:**
  - El suelo es un único cubo de 2000×2000 unidades (8 km por lado).
  - Las dunas son columnas de bloques en un anillo de 89×89 celdas, que se aplanan hacia lo lejos.
  - La niebla exponencial (densidad 0.0065 por unidad) funde el suelo con el horizonte del skybox. No hay zócalo ni bordes visibles.
- **Las Noches:**
  - El tambor, la cornisa y la cúpula usan **celdas de cuarto de bloque** para que las curvas se vean suaves.
  - Las caras que miran hacia adentro usan el material "cielo falso".
  - El tambor tiene una banda de ventanas encendidas.
  - Un corte en cuña (180°–255°) deja ver el interior.
- **Torres:**
  - Son cilindros de cuarto de bloque cuyo radio crece cerca de la base: `r(y) = r0 + 3·e^(−y/3.5)`.
  - Tienen rendijas de ventana encendidas.
  - Los niveles iguales y consecutivos de una celda se unen en un solo cubo alto, para no tener cientos de miles de objetos.
- **Calzada:** losas de obsidiana de 2 bloques, con bordillos de concreto.
- **Árboles de cuarzo:** troncos de cubos alargados y rotados que se dividen en ramas dos veces.
- **Zangetsu:** tres cubos rotados (hoja de acero, lomo de obsidiana y mango vendado), a escala real.

### Técnicas de raytracing

1. **Cámara:** cámara estenopeica con FOV de 45°. El zoom mueve la cámara sobre la línea de vista (`camera.rs`).
2. **Rotación del diorama:** cada rayo se lleva al espacio del diorama con la inversa de `rot_y(yaw)`, alrededor del pivote (centro de Las Noches). La BVH se construye una sola vez y sigue siendo válida; el cielo y la niebla se muestrean con la dirección en el mundo (`renderer.rs::View`).
3. **Intersección rayo–cubo (slabs):**
   - Devuelve la cara de entrada, o la de salida si el rayo nace dentro, algo necesario al salir de un cristal.
   - La normal apunta hacia afuera.
   - Las UV van por cara y en unidades de mundo, orientadas para no quedar en espejo (probado en `tests/optica.rs`).
   - Los cubos rotados transforman el rayo a su espacio local; la normal vuelve con la misma rotación.
4. **BVH:** hojas de ≤4 objetos, con unos 23 mil cubos. El recorrido visita primero el hijo más cercano, y la prueba rayo-caja maneja rayos paralelos a un eje.
5. **Iluminación:**
   - **Luces direccionales:**
     - el resplandor lunar difuso de las nubes, con **sombras suaves**;
     - el contraluz de la luna.
   - **Luces puntuales:** el cielo falso que se derrama por el corte, y la luz de la puerta que cae sobre la calzada.
   - Cada luz puntual tiene un alcance máximo, y no se lanzan rayos de sombra hacia luces que no aportan.
   - Los materiales emisivos (ventanas, cielo falso) se ven encendidos, pero **no** iluminan por sí mismos: la luz que cae sobre la escena viene de las luces puntuales.
6. **Sombras a través de cristal:** el rayo de sombra atraviesa materiales transparentes multiplicando por `transparencia × tinte` (sombras tintadas, sin cáusticas).
7. **Reflexión y refracción recursivas** (máx. 6 rebotes en el render final):
   - Snell con `n1/n2` según si el rayo entra o sale (se detecta con `d·n > 0` y se invierte la normal).
   - **Reflexión interna total** cuando `sin²θt > 1`.
   - **Fresnel de Schlick** para repartir reflexión y transmisión.
   - La cara interior de un cristal no recibe luz directa.
8. **Niebla:** `color = mezcla(color, horizonte_del_skybox, 1 − e^(−d·densidad))`. Se aplica a todos los rayos, así que también a los reflejos y las refracciones.
9. **Autointersección:** el origen de los rayos secundarios se desplaza `±N·0.002` según el lado.
10. **Skybox:** cubemap de 6 caras con convención de OpenGL, muestreado con filtro bilineal fijo al borde de cada cara. Contiene:
    - nubes de tormenta;
    - una luna creciente fina con destello;
    - estrellas tenues;
    - la Garganta.
11. **Color:**
    - Las texturas se convierten de sRGB a lineal una vez al cargarlas.
    - Todo el cálculo es lineal.
    - Al final se aplica exposición, tone mapping ACES y conversión a sRGB **una sola vez** (sin doble gamma).
12. **Antialiasing y determinismo:** jitter subpíxel con un generador PCG sembrado por (x, y, muestra). El mismo comando produce exactamente la misma imagen.
13. **Paralelismo:** `std::thread::scope` con un contador atómico de filas.

## Rendimiento

Medido con `cargo run --release -- --bench` en la laptop de desarrollo: Intel Core i7-1185G7 (4 núcleos / 8 hilos), 32 GB RAM, **solo CPU**, vista héroe, 1 muestra por píxel, 22,971 cubos.

| Modo | Resolución | Rebotes | Sombras suaves | ms/cuadro |
|---|---|---:|:---:|---:|
| Vista previa (en movimiento) | 320×180 | 2 | no | PENDIENTE |
| Refinado | 640×360 | 6 | sí | PENDIENTE |
| Final | 1280×720 | 6 | sí | PENDIENTE |

En la ventana (calidad 2), mientras se mueve se renderiza a 426×240 y luego se acumulan muestras a 1280×720. Video (1800 cuadros, 1280×720, 6 spp): PENDIENTE.

## Verificación

- `cargo test --release`: 8 pruebas:
  - incidencia normal sin desviación
  - ley de Snell a 40°
  - reflexión interna total (60° sí, 30° no)
  - Fresnel dentro de [0,1]
  - entrada y salida del cubo
  - UV sin espejo en las 4 caras laterales
  - ida y vuelta del skybox
  - todo lo que sobresale del suelo cabe dentro del límite de zoom
- `--diag` (imágenes abajo):
  - Una lámina de cuarzo de frente no desplaza el tablero; girada 40°, sí lo desplaza.
  - Un cubo de cuarzo girado muestra reflexión interna.
  - Un espejo refleja el tablero y el cielo (skybox en rayos secundarios).
  - El cubo con la letra "F" confirma la orientación de las UV.

![Diagnóstico de frente](docs/img/diag_frente.png)
![Diagnóstico lateral](docs/img/diag_lateral.png)

## Vistas

![Cuatro ángulos](docs/img/angulos.png)
![Zoom cerca y lejos](docs/img/zoom.png)

## Limitaciones conocidas

- La ventana interactiva es solo para Windows (API Win32 por FFI). En otros sistemas se pueden usar `--render` y `--video`.
- Es un raytracer tipo Whitted:
  - No hay iluminación global ni cáusticas.
  - La luz que atraviesa el cuarzo se aproxima con sombras tintadas.
  - Los materiales emisivos no iluminan por sí mismos; la luz la aportan luces puntuales colocadas en la puerta y dentro de la cúpula.
- Cuando se acaba la profundidad de recursión (o el aporte baja del 1%), el último rayo reflejado o refractado toma el color del skybox. En calidad 1 (2 rebotes) esto se nota en los cristales.
- No hay *mipmapping*: a lo lejos las texturas podrían "titilar". La niebla y las muestras por píxel lo disimulan.
- El PNG que escribe el proyecto usa deflate sin compresión, así que los archivos son más grandes que un PNG normal.

## Recursos y licencias

- **Todo el arte es original**: texturas, skybox, luna, nubes y Garganta se generan con `src/bin/gen_textures.rs`. No se usaron imágenes de terceros.
- La escena es un homenaje (fan art) a *Bleach* de Tite Kubo. Los nombres Hueco Mundo, Las Noches, Garganta y Zangetsu pertenecen a su obra; aquí no se usa ningún recurso de la serie.
- **Herramienta de desarrollo (no es dependencia del proyecto):** el video MP4 y la vista previa GIF se armaron a partir de los cuadros BMP con **FFmpeg**. El programa no la usa ni la necesita para compilar o ejecutarse.
