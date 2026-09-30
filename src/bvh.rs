//! BVH (jerarquía de volúmenes envolventes) sobre los objetos del diorama.
//! Se construye una sola vez en espacio del diorama: rotar el diorama solo
//! transforma los rayos, así que la BVH sigue siendo válida.

use crate::geometry::{Hit, Object, Ray};
use crate::math::Vec3;

struct Node {
    min: Vec3,
    max: Vec3,
    /// Hoja: `count > 0` y `first` indexa en `order`. Interno: hijos en `first` y `first + 1`.
    first: u32,
    count: u32,
}

pub struct Bvh {
    nodes: Vec<Node>,
    order: Vec<u32>,
}

const LEAF_SIZE: usize = 4;

impl Bvh {
    pub fn build(objects: &[Object]) -> Bvh {
        let bounds: Vec<(Vec3, Vec3)> = objects.iter().map(|o| o.bounds()).collect();
        let centers: Vec<Vec3> = bounds.iter().map(|(a, b)| (*a + *b) * 0.5).collect();
        let mut bvh = Bvh { nodes: Vec::new(), order: (0..objects.len() as u32).collect() };
        bvh.nodes.push(Node { min: Vec3::ZERO, max: Vec3::ZERO, first: 0, count: 0 });
        bvh.subdivide(0, 0, objects.len(), &bounds, &centers);
        bvh
    }

    fn subdivide(&mut self, node: usize, start: usize, end: usize, bounds: &[(Vec3, Vec3)], centers: &[Vec3]) {
        let mut bmin = Vec3::splat(f32::INFINITY);
        let mut bmax = Vec3::splat(f32::NEG_INFINITY);
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
        let ext = cmax - cmin;
        let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
        if n <= LEAF_SIZE || ext.axis(axis) < 1e-6 {
            self.nodes[node].first = start as u32;
            self.nodes[node].count = n as u32;
            return;
        }
        // División por la mediana del eje más largo de los centros.
        self.order[start..end].sort_unstable_by(|&a, &b| {
            centers[a as usize].axis(axis).total_cmp(&centers[b as usize].axis(axis))
        });
        let mid = start + n / 2;
        let left = self.nodes.len();
        self.nodes.push(Node { min: Vec3::ZERO, max: Vec3::ZERO, first: 0, count: 0 });
        self.nodes.push(Node { min: Vec3::ZERO, max: Vec3::ZERO, first: 0, count: 0 });
        self.nodes[node].first = left as u32;
        self.nodes[node].count = 0;
        self.subdivide(left, start, mid, bounds, centers);
        self.subdivide(left + 1, mid, end, bounds, centers);
    }

    #[inline]
    fn hit_aabb(min: Vec3, max: Vec3, o: Vec3, inv: Vec3, t_max: f32) -> Option<f32> {
        let tx1 = (min.x - o.x) * inv.x;
        let tx2 = (max.x - o.x) * inv.x;
        let ty1 = (min.y - o.y) * inv.y;
        let ty2 = (max.y - o.y) * inv.y;
        let tz1 = (min.z - o.z) * inv.z;
        let tz2 = (max.z - o.z) * inv.z;
        let tmin = tx1.min(tx2).max(ty1.min(ty2)).max(tz1.min(tz2));
        let tmax = tx1.max(tx2).min(ty1.max(ty2)).min(tz1.max(tz2));
        if tmax >= tmin.max(0.0) && tmin < t_max { Some(tmin) } else { None }
    }

    /// Intersección más cercana en (t_min, t_max).
    pub fn intersect(&self, objects: &[Object], ray: &Ray, t_min: f32, mut t_max: f32) -> Option<Hit> {
        let inv = Vec3::new(1.0 / ray.dir.x, 1.0 / ray.dir.y, 1.0 / ray.dir.z);
        let mut best: Option<Hit> = None;
        let mut stack = [0u32; 64];
        let mut sp = 0usize;
        if Self::hit_aabb(self.nodes[0].min, self.nodes[0].max, ray.origin, inv, t_max).is_none() {
            return None;
        }
        stack[sp] = 0;
        sp += 1;
        while sp > 0 {
            sp -= 1;
            let node = &self.nodes[stack[sp] as usize];
            if node.count > 0 {
                for &i in &self.order[node.first as usize..(node.first + node.count) as usize] {
                    if let Some(h) = objects[i as usize].intersect(ray, t_min, t_max) {
                        t_max = h.t;
                        best = Some(h);
                    }
                }
                continue;
            }
            // Visitamos primero el hijo más cercano (se apila al final).
            let (a, b) = (node.first, node.first + 1);
            let ta = Self::hit_aabb(self.nodes[a as usize].min, self.nodes[a as usize].max, ray.origin, inv, t_max);
            let tb = Self::hit_aabb(self.nodes[b as usize].min, self.nodes[b as usize].max, ray.origin, inv, t_max);
            match (ta, tb) {
                (Some(x), Some(y)) => {
                    let (near, far) = if x <= y { (a, b) } else { (b, a) };
                    stack[sp] = far;
                    stack[sp + 1] = near;
                    sp += 2;
                }
                (Some(_), None) => {
                    stack[sp] = a;
                    sp += 1;
                }
                (None, Some(_)) => {
                    stack[sp] = b;
                    sp += 1;
                }
                _ => {}
            }
        }
        best
    }
}
