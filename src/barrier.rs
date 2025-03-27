use hoydedata::Coord;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fmt::Display;

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(transparent)]
pub struct Barrier {
    pub points: Vec<Coord>,
}

impl Barrier {
    pub fn new() -> Self {
        Self {
            points: vec![],
        }
    }

    pub fn from_vec(points: Vec<Coord>) -> Self {
        Self {
            points: points,
        }
    }

    pub fn add_point(&mut self, p: Coord) {
        self.points.push(p);
    }

    pub fn update_point(&mut self, i: usize, p: Coord) {
        self.points[i] = p;
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    // Calculates the signed triangle area formed by three points
    fn triangle_area(a: &Coord, b: &Coord, c: &Coord) -> f32 {
        return (b.e - a.e) * (c.n - a.n) - (c.e - a.e) * (b.n - a.n);
    }

    // Check whether segment (b1 - b2) crosses the line running through
    // the points a1 and a2
    fn is_crossing_line(a1: &Coord, a2: &Coord, b1: &Coord, b2: &Coord)
                        -> bool {
        let area_b1 = Barrier::triangle_area(a1, a2, b1);
        let area_b2 = Barrier::triangle_area(a1, a2, b2);

        (area_b1 < 0.0 && area_b2 > 0.0) || (area_b1 > 0.0 && area_b2 < 0.0)
    }

    // Check whether segment (p1 - p2) crosses a segment of the barrier
    fn is_crossing_segment(&self, i: usize, p1: &Coord, p2: &Coord) -> bool {
        let a1 = &self.points[i];
        let a2 = &self.points[i + 1];

        return Barrier::is_crossing_line(a1, a2, p1, p2) &&
            Barrier::is_crossing_line(p1, p2, a1, a2);
    }

    // Check whether a line segment crosses the barrier
    pub fn is_crossing(&self, p1: &Coord, p2: &Coord) -> bool {
        let len = self.points.len();

        for i in 0..len - 1 {
            if self.is_crossing_segment(i, p1, p2) {
                return true;
            }
        }

        false
    }

    pub fn distance_from_segment_sq(&self, i: usize, p: &Coord) -> f32 {
        let p1 = &self.points[i];
        let p2 = &self.points[i + 1];

        let d1 = *p - *p1;
        let d2 = *p2 - *p1;

        let dot = d1.dot(&d2);
        let abs_sq = d2.abs_sq();

        // Projection of point down to line segment [p1..p2] -> [0..1]
        let mut param = -1.0;

        if abs_sq != 0.0 {
            param = dot/abs_sq;
        }

        // Find closest point on segment
        let pp = if param < 0.0 {
            // p is below p1 -> p1 is nearest point
            *p1
        }
        else if param > 1.0 {
            // p is above p2 -> p2 is nearest point
            *p2
        }
        else {
            // p is between p1 and p2 -> nearest point is on segment
            *p1 + d2*param
        };

        return (*p - pp).abs_sq();
    }

    pub fn distance_sq(&self, p: &Coord) -> f32 {
        let mut dsq = f32::INFINITY;
        let len = self.points.len();

        for i in 0..len - 1 {
            dsq = dsq.min(self.distance_from_segment_sq(i, p));
        }

        dsq
    }
}

impl Display for Barrier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let str = &self.points.iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join(", ");

        write!(formatter, "{}", str)?;
        Ok(())
    }
}
