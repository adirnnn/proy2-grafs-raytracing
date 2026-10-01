//! bvh (jerarquía de volúmenes envolventes) sobre los objetos del diorama.
//!
//! en vez de probar cada rayo contra todos los objetos de la escena, los agrupamos
//! en un árbol binario de cajas alineadas a los ejes (aabb). si un rayo no toca la
//! caja de un nodo, tampoco puede tocar nada de lo que hay adentro, así que nos
//! saltamos esa rama completa. con esto el costo pasa de lineal a aproximadamente
//! logarítmico en el número de objetos.
//!
//! la construimos una sola vez en espacio del diorama: rotar el diorama solo
//! transforma los rayos (ver renderer), así que la bvh sigue siendo válida sin
//! reconstruirla en cada cuadro.

use crate::geometry::{Hit, Object, Ray};
use crate::math::Vec3;

/// un nodo del árbol. todos los nodos viven en un solo arreglo plano (`Bvh::nodes`)
/// y se referencian por índice, lo cual es más amigable con la caché que usar punteros.
struct Node {
    /// esquina mínima de la caja que envuelve todo lo que cuelga de este nodo.
    min: Vec3,
    /// esquina máxima de esa misma caja.
    max: Vec3,
    /// hoja: `count > 0` y `first` indexa en `order` (primer objeto de la hoja).
    /// interno: `count` vale cero y los hijos están en `first` y `first + 1`.
    first: u32,
    /// cantidad de objetos de la hoja; cero indica que el nodo es interno.
    count: u32,
}

/// la jerarquía completa: arreglo de nodos (el nodo cero es la raíz) y una
/// permutación de los índices de objetos que reordenamos al construir, para que
/// los objetos de cada hoja queden contiguos.
pub struct Bvh {
    /// todos los nodos del árbol; los dos hijos de un nodo interno siempre van seguidos.
    nodes: Vec<Node>,
    /// índices de objetos ordenados de forma que cada hoja es un rango continuo.
    order: Vec<u32>,
}

/// máximo de objetos por hoja. si un rango tiene esta cantidad o menos, ya no lo partimos.
const LEAF_SIZE: usize = 4;

impl Bvh {
    /// construye la bvh para la lista de objetos.
    /// `objects` son las primitivas de la escena; devuelve el árbol listo para intersecar.
    pub fn build(objects: &[Object]) -> Bvh {
        // precalculamos la caja de cada objeto y su centro, porque los vamos a consultar muchas veces
        let bounds: Vec<(Vec3, Vec3)> = objects.iter().map(|o| o.bounds()).collect();
        let centers: Vec<Vec3> = bounds.iter().map(|(a, b)| (*a + *b) * 0.5).collect();
        // al inicio el orden es la identidad 0, 1, 2, ...
        let mut bvh = Bvh { nodes: Vec::new(), order: (0..objects.len() as u32).collect() };
        // metemos la raíz vacía; subdivide se encarga de llenar su caja y sus hijos
        bvh.nodes.push(Node { min: Vec3::ZERO, max: Vec3::ZERO, first: 0, count: 0 });
        bvh.subdivide(0, 0, objects.len(), &bounds, &centers);
        bvh
    }

    /// construye recursivamente el nodo `node` con los objetos `order[start..end]`.
    /// `bounds` y `centers` son las cajas y centros precalculados de cada objeto.
    fn subdivide(&mut self, node: usize, start: usize, end: usize, bounds: &[(Vec3, Vec3)], centers: &[Vec3]) {
        // empezamos con cajas "invertidas" (mínimo en infinito, máximo en menos infinito)
        // para que el primer objeto que agreguemos las ajuste de inmediato
        let mut bmin = Vec3::splat(f32::INFINITY);
        let mut bmax = Vec3::splat(f32::NEG_INFINITY);
        // caja de los centros: la usamos para decidir por qué eje partir
        let mut cmin = bmin;
        let mut cmax = bmax;
        for &i in &self.order[start..end] {
            let (a, b) = bounds[i as usize];
            bmin = bmin.min(a);
            bmax = bmax.max(b);
            cmin = cmin.min(centers[i as usize]);
            cmax = cmax.max(centers[i as usize]);
        }
        self.nodes[node].min = bmin;
        self.nodes[node].max = bmax;
        let n = end - start;
        // escogemos el eje donde los centros están más esparcidos
        let ext = cmax - cmin;
        let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
        // se vuelve hoja si ya hay pocos objetos o si todos los centros coinciden
        // (en ese caso partir no separaría nada y la recursión no terminaría)
        if n <= LEAF_SIZE || ext.axis(axis) < 1e-6 {
            self.nodes[node].first = start as u32;
            self.nodes[node].count = n as u32;
            return;
        }
        // división por la mediana del eje más largo de los centros:
        // ordenamos el rango por la coordenada de su centro en ese eje
        // (total_cmp da un orden total aunque aparezca algún nan)
        self.order[start..end].sort_unstable_by(|&a, &b| {
            centers[a as usize].axis(axis).total_cmp(&centers[b as usize].axis(axis))
        });
        // la mitad izquierda va al primer hijo y la derecha al segundo, así el árbol queda balanceado
        let mid = start + n / 2;
        // reservamos los dos hijos juntos al final del arreglo
        let left = self.nodes.len();
        self.nodes.push(Node { min: Vec3::ZERO, max: Vec3::ZERO, first: 0, count: 0 });
        self.nodes.push(Node { min: Vec3::ZERO, max: Vec3::ZERO, first: 0, count: 0 });
        // el nodo actual pasa a ser interno: apunta al hijo izquierdo y count queda en cero
        self.nodes[node].first = left as u32;
        self.nodes[node].count = 0;
        self.subdivide(left, start, mid, bounds, centers);
        self.subdivide(left + 1, mid, end, bounds, centers);
    }

    /// prueba de intersección rayo contra caja alineada (método de slabs).
    /// `min` y `max` son las esquinas de la caja, `o` el origen del rayo, `inv` el
    /// inverso de cada componente de la dirección y `t_max` la distancia del mejor
    /// impacto encontrado hasta ahora. devuelve la distancia de entrada a la caja
    /// (puede ser negativa si el origen está adentro) o `None` si no hay cruce útil.
    #[inline]
    fn hit_aabb(min: Vec3, max: Vec3, o: Vec3, inv: Vec3, t_max: f32) -> Option<f32> {
        // cada par de planos paralelos (un "slab") da un intervalo de t donde el rayo está
        // entre ellos: t igual a (plano menos origen) por el inverso de la dirección.
        // si una componente de la dirección es cero, inv vale infinito y (plano menos o) puede
        // ser cero: cero por infinito da nan. en ese caso el rayo está sobre el plano y lo
        // tratamos como "dentro" del slab, dándole el intervalo más amplio posible.
        let slab = |lo: f32, hi: f32, o: f32, inv: f32| {
            let (a, b) = ((lo - o) * inv, (hi - o) * inv);
            (if a.is_nan() { f32::NEG_INFINITY } else { a }, if b.is_nan() { f32::INFINITY } else { b })
        };
        let (tx1, tx2) = slab(min.x, max.x, o.x, inv.x);
        let (ty1, ty2) = slab(min.y, max.y, o.y, inv.y);
        let (tz1, tz2) = slab(min.z, max.z, o.z, inv.z);
        // el rayo está dentro de la caja solo donde está dentro de los tres slabs a la vez:
        // la entrada es el mayor de los mínimos y la salida el menor de los máximos
        let tmin = tx1.min(tx2).max(ty1.min(ty2)).max(tz1.min(tz2));
        let tmax = tx1.max(tx2).min(ty1.max(ty2)).min(tz1.max(tz2));
        // hay cruce si el intervalo no está vacío, si la salida no queda detrás del origen
        // y si la entrada está más cerca que el mejor impacto que ya tenemos
        if tmax >= tmin.max(0.0) && tmin < t_max { Some(tmin) } else { None }
    }

    /// busca la intersección más cercana del rayo con algún objeto, dentro de (t_min, t_max).
    /// `objects` debe ser la misma lista con la que se construyó la bvh.
    /// devuelve el `Hit` más cercano o `None` si el rayo no toca nada.
    pub fn intersect(&self, objects: &[Object], ray: &Ray, t_min: f32, mut t_max: f32) -> Option<Hit> {
        // precalculamos el inverso de la dirección una vez para no dividir en cada caja
        let inv = Vec3::new(1.0 / ray.dir.x, 1.0 / ray.dir.y, 1.0 / ray.dir.z);
        let mut best: Option<Hit> = None;
        // recorrido iterativo con una pila fija en lugar de recursión (64 niveles sobran)
        let mut stack = [0u32; 64];
        let mut sp = 0usize;
        // si el rayo ni siquiera toca la caja raíz, terminamos de una vez
        if Self::hit_aabb(self.nodes[0].min, self.nodes[0].max, ray.origin, inv, t_max).is_none() {
            return None;
        }
        stack[sp] = 0;
        sp += 1;
        while sp > 0 {
            // sacamos el nodo del tope de la pila
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            if node.count > 0 {
                // hoja: probamos cada objeto de verdad
                for &i in &self.order[node.first as usize..(node.first + node.count) as usize] {
                    if let Some(h) = objects[i as usize].intersect(ray, t_min, t_max) {
                        // achicamos t_max: de aquí en adelante solo nos interesa algo más cercano,
                        // y eso también hace que hit_aabb descarte más cajas
                        t_max = h.t;
                        best = Some(h);
                    }
                }
                continue;
            }
            // nodo interno: probamos las cajas de ambos hijos.
            // visitamos primero el hijo más cercano (se apila al final para salir primero),
            // así encontramos pronto un impacto cercano y podamos más ramas
            let (a, b) = (node.first, node.first + 1);
            let ta = Self::hit_aabb(self.nodes[a as usize].min, self.nodes[a as usize].max, ray.origin, inv, t_max);
            let tb = Self::hit_aabb(self.nodes[b as usize].min, self.nodes[b as usize].max, ray.origin, inv, t_max);
            match (ta, tb) {
                (Some(x), Some(y)) => {
                    // ambos hijos se cruzan: el lejano abajo y el cercano arriba
                    let (near, far) = if x <= y { (a, b) } else { (b, a) };
                    stack[sp] = far;
                    stack[sp + 1] = near;
                    sp += 2;
                }
                (Some(_), None) => {
                    // solo el hijo izquierdo
                    stack[sp] = a;
                    sp += 1;
                }
                (None, Some(_)) => {
                    // solo el hijo derecho
                    stack[sp] = b;
                    sp += 1;
                }
                // ninguno: descartamos esta rama completa
                _ => {}
            }
        }
        best
    }
}
