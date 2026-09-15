// Copyright 2024 the Velato Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use kurbo::{PathEl, Point};

/// Helper trait for converting cubic splines to paths.
pub trait SplineToPath {
    fn get(&self, index: usize) -> Point;
    fn len(&self) -> usize;

    fn to_path(&self, is_closed: bool, path: &mut Vec<PathEl>) -> Option<()> {
        use PathEl::*;
        let len = self.len();
        if len == 0 {
            return None;
        }
        let n_vertices = len / 3;
        let close = is_closed && n_vertices != 0;
        path.reserve(n_vertices.max(1) + if close { 2 } else { 0 });
        let first = self.get(0);
        path.push(MoveTo(first));
        let mut p0 = first;
        let mut add_element = |from_vertex, to_vertex, p1: Point| {
            let from_index = 3 * from_vertex;
            let to_index = 3 * to_vertex;
            let c0 = self.get(from_index + 2);
            let c1 = self.get(to_index + 1);
            push_segment(path, p0, c0, p1, c1);
            p0 = p1;
        };
        for i in 1..n_vertices {
            add_element(i - 1, i, self.get(3 * i));
        }
        if close {
            add_element(n_vertices - 1, 0, first);
            path.push(ClosePath);
        }
        Some(())
    }
}

/// Converts a static spline to a path.
impl SplineToPath for &'_ [Point] {
    fn len(&self) -> usize {
        self.as_ref().len()
    }

    fn get(&self, index: usize) -> Point {
        self[index]
    }
}

/// Produces a path by lerping between two sets of points.
impl SplineToPath for (&'_ [Point], &'_ [Point], f64) {
    fn len(&self) -> usize {
        self.0.len().min(self.1.len())
    }

    fn get(&self, index: usize) -> Point {
        // TODO: This definitely shouldn't be a lerp
        self.0[index].lerp(self.1[index], self.2)
    }

    fn to_path(&self, is_closed: bool, path: &mut Vec<PathEl>) -> Option<()> {
        let len = self.len();
        if len == 0 {
            return None;
        }
        let n_vertices = len / 3;
        let close = is_closed && n_vertices != 0;
        path.reserve(n_vertices.max(1) + if close { 2 } else { 0 });
        if n_vertices == 0 {
            path.push(PathEl::MoveTo(self.get(0)));
            return Some(());
        }

        let mut vertices = self.0.chunks_exact(3).zip(self.1.chunks_exact(3));
        let (from, to) = vertices.next()?;
        let first = from[0].lerp(to[0], self.2);
        let first_in = from[1].lerp(to[1], self.2);
        let mut previous = first;
        let mut outgoing = from[2].lerp(to[2], self.2);
        path.push(PathEl::MoveTo(first));
        for (from, to) in vertices {
            let point = from[0].lerp(to[0], self.2);
            let incoming = from[1].lerp(to[1], self.2);
            push_segment(path, previous, outgoing, point, incoming);
            previous = point;
            outgoing = from[2].lerp(to[2], self.2);
        }
        if close {
            push_segment(path, previous, outgoing, first, first_in);
            path.push(PathEl::ClosePath);
        }
        Some(())
    }
}

fn push_segment(path: &mut Vec<PathEl>, p0: Point, mut c0: Point, p1: Point, mut c1: Point) {
    c0.x += p0.x;
    c0.y += p0.y;
    c1.x += p1.x;
    c1.y += p1.y;
    if c0 == p0 && c1 == p1 {
        path.push(PathEl::LineTo(p1));
    } else {
        path.push(PathEl::CurveTo(c0, c1, p1));
    }
}
