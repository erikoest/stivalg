use crate::field::Field;
use crate::graph::Graph;
use crate::params::Params;

use core::slice::Iter;
use geo_types::Point;
use gpx::{Gpx, GpxVersion, Metadata, Track, TrackSegment, Waypoint};
use hoydedata::{Atlas, Coord};
use std::fmt;
use std::{fs::File, io::BufWriter};
use std::io::BufReader;

#[derive(Clone)]
pub struct Segment {
    pub a: Coord,
    pub b: Coord,
}

impl Segment {
    pub fn new(a: Coord, b: Coord) -> Self {
        Self {
            a: a,
            b: b,
        }
    }

    pub fn fields(&self) -> SegmentIterator {
        return SegmentIterator::new(self);
    }

    pub fn len(&self) -> f32 {
        return (self.b - self.a).abs();
    }

    pub fn _time_by_steepness(s: f32, abs: f32) -> f32 {
        if s < 0.0 {
            return 1.0 + 4.0*abs;
        }
        else {
            // 1 s/m + 3600s/300hm
            return 1.0 + 20.0*s + 4.0*abs;
        }
    }

    pub fn time_by_steepness(s: f32, abs: f32) -> f32 {
        // The functions is made by points of tan(s) -> time/distance.
        // Between the points, the value is interpolated.
        let (s1, s2, t1, t2) = match s {
            x if (..-1.0).contains(&x)       => (-2.0, -1.0, 40.0, 15.0),  // -63 - -40
            x if (-1.0..-0.83).contains(&x)  => (-1.0, -0.83, 15.0, 3.0),  // -45 - -40
            x if (-0.83..-0.58).contains(&x) => (-0.83, -0.58, 3.0, 1.2), // -40 - -30
            x if (-0.58..-0.36).contains(&x) => (-0.58, -0.36, 1.2, 0.7), // -30 - -20
            x if (-0.36..-0.18).contains(&x) => (-0.36, -0.12, 0.7, 0.5), // -20 - -10
            x if (-0.18..0.0).contains(&x)   => (-0.18, 0.0, 0.5, 1.2),   // -10 - 0
            x if (0.0..0.18).contains(&x)    => (0.0, 0.18, 1.2, 1.7),    //  0 - 10
            x if (0.18..0.36).contains(&x)   => (0.18, 0.36, 1.7, 2.5),   //  10 - 20
            x if (0.36..0.58).contains(&x)   => (0.36, 0.58, 2.5, 4.0),   //  20 - 30
            x if (0.58..0.83).contains(&x)   => (0.58, 0.83, 4.0, 10.0),  //  30 - 40
            x if (0.83..1.0).contains(&x)    => (0.83, 1.0, 10.0, 60.0),  //  40 - 45
            x if (1.0..).contains(&x)        => (1.0, 2.0, 60.0, 600.0),   //  45 - 63
            _                                => (1.0, 2.0, 60.0, 10000.0),
        };

        return (t2 - t1)*(s - s1)/(s2 - s1) + t1 + 5.0*abs;
    }

    // Graf: 2601 vx, 5100 edges

    // Calculate cost of walking the segment. Input is an atlas of height
    // maps. Output is a cost value.
    pub fn time(&self, atlas: &Atlas) -> Option<f32> {
        let mut time = 0.0;

        let (be, bn, ae, an) = (self.b.e, self.b.n, self.a.e, self.a.n);
        let r = ((be - ae)*(be - ae) + (bn - an)*(bn - an)).sqrt();
        let de = (be - ae)/r;
        let dn = (bn - an)/r;

        for (f, l) in self.fields() {
            let (_, dx, dy) = atlas.lookup_with_gradient(&f.into()).unwrap();
            // If absolute gradient is too high (45 degrees), return None
            let abs = dx*dx + dy*dy;
            if abs > 1.0 {
                return None;
            }

            let s = de*dx + dn*dy;
            time += l*Segment::time_by_steepness(s, abs);
        }

        return Some(time);
    }

    // Calculate uphill height meters along the segment
    pub fn height(&self, atlas: &Atlas) -> f32 {
        let mut height = 0.0;

        let (be, bn, ae, an) = (self.b.e, self.b.n, self.a.e, self.a.n);
        let r = ((be - ae)*(be - ae) + (bn - an)*(bn - an)).sqrt();

        if r == 0.0 {
            return 0.0;
        }

        let de = (be - ae)/r;
        let dn = (bn - an)/r;

        for (f, l) in self.fields() {
            let (_, dx, dy) = atlas.lookup_with_gradient(&f.into()).unwrap();
            let s = de*dx + dn*dy;
            height += if s < 0.0 { 0.0 } else { s*l };
        }

        return height;
    }
}

impl fmt::Display for Segment {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(format_args!("{} -> {}", self.a, self.b))
    }
}

// Iterator yielding (Field, f32) by traversing a segment from point A to B
pub struct SegmentIterator {
    p:       Segment,       // Segment to iterate
    some_cf: Option<Field>, // Current field
    cin:     Coord          // Start coordinate of current field
}

impl SegmentIterator {
    pub fn new(p: &Segment) -> Self {
        let af = Field::from(p.a);

        Self {
            p: p.clone(),
            some_cf: Some(af),
            cin: p.a,
        }
    }
}

impl Iterator for SegmentIterator {
    type Item = (Field, f32);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cf) = self.some_cf {
            let length;
            if let Some((nx_cin, nx_cf)) = cf.crossing(&self.cin, &self.p.b) {
                self.some_cf.replace(nx_cf);
                length = (nx_cin - self.cin).abs();
                self.cin = nx_cin;
            }
            else {
                self.some_cf.take();
                length = (self.p.b - self.cin).abs();
            }

            return Some((cf, length));
        }
        else {
            return None;
        }
    }
}

#[derive(Clone, Debug)]
pub struct Path {
    points: Vec<Coord>,
}

impl Path {
    pub fn new() -> Self {
        Self {
            points: vec!(),
        }
    }

    // Create path from a vector of points. First, use graph shortest path, i
    // order to establish a start path. Then optimize the path using iterative
    // relaxation.
    pub fn from_points(params: &Params, atlas: &Atlas) -> Option<Self> {
        let points = &params.points;
        let len = points.len();

        assert!(len >= 2);
        let mut path = Path::new();

        for i in 0..len - 1 {
            // Find a start path using a shortest path algorithm over a graph
            // of points in the area between the start and end points.
            let mut g = Graph::new(points[i], points[i + 1], params);
            println!("Building first pass graph...");
            g.build_graph_from_end_points(atlas);
            println!("First pass graph: {} nodes, {} edges", g.num_nodes(),
                     g.num_edges());
            println!("Finding shortest path...");

            if let Some(p) = g.shortest_path() {
                println!("First pass path: {} points, {}m", p.points.len(),
                         p.len());
                let mut g2 = Graph::new(points[i], points[i + 1], params);
                println!("Building second pass graph...");
                g2.build_graph_from_path(&p, atlas);
                println!("Second pass graph: {} nodes, {} edges",
                         g2.num_nodes(), g2.num_edges());
                println!("Finding shortest path...");

                if let Some(mut p2) = g2.shortest_path() {
                    println!("Second pass path: {} points, {}m",
                             p2.points.len(), p2.len());
                    println!("Local optimization...");
                    p2.optimize(atlas);
                    println!("Final path: {} points, {}m", p2.points.len(),
                             p2.len());
                    path.append(&mut p2);
                }
            }
            else {
                return None;
            }
        }

        return Some(path);
    }

    pub fn push(&mut self, c: Coord) {
        self.points.push(c);
    }

    pub fn append(&mut self, other: &mut Path) {
        if other.points.len() != 0 {
            if self.points.len() == 0 {
                self.points = other.points.drain(..).collect();
            }
            else {
                assert!(self.points[self.points.len() - 1] == other.points[0]);
                self.points.pop();
                self.points.append(&mut other.points);
            }
        }
    }

    fn tripoint_time(&self, c1: Coord, c2: Coord, c3: Coord, atlas: &Atlas)
                     -> f32 {
        if let Some(t1) = Segment::new(c1, c2).time(atlas) {
            if let Some(t2) = Segment::new(c2, c3).time(atlas) {
                return t1 + t2;
            }
        }

        return f32::INFINITY;
    }

    // Optimize path using iterative relaxation.
    pub fn optimize(&mut self, atlas: &Atlas) {
        println!("Improving path iteratively.");
        let de = Coord::new(4.0, 0.0);
        let dn = Coord::new(0.0, 4.0);
        let mut time = self.calculate_time(atlas);
        println!("Before adjustments: Time {}, points {}", time,
                 self.points.len());

        loop {
            let len = self.points.len();

            for i in 1..len - 1 {
                // Current, previous and next point
                let c = self.points[i];
                let p = self.points[i - 1];
                let n = self.points[i + 1];

                let t0 = self.tripoint_time(p, c, n, atlas);
                let te1 = self.tripoint_time(p, c - de, n, atlas);
                let te2 = self.tripoint_time(p, c + de, n, atlas);
                let tn1 = self.tripoint_time(p, c - dn, n, atlas);
                let tn2 = self.tripoint_time(p, c + dn, n, atlas);

                let mut dc_n = Coord::new(0.0, 0.0);
                let mut dc_e = Coord::new(0.0, 0.0);

                if te1.is_finite() {
                    dc_e += de*(te1 - t0);
                }
                if te2.is_finite() {
                    dc_e += de*(t0 - te2);
                }
                if tn1.is_finite() {
                    dc_n += dn*(tn1 - t0);
                }
                if tn2.is_finite() {
                    dc_n += dn*(t0 - tn2);
                }

                let mut dc = (dc_e + dc_n)*16.0;

                if dc.abs() == 0.0 {
                    continue;
                }

                if dc.abs() > 20.0 {
                    dc = dc.normalize()*20.0;
                }

                let mut tmin = t0;

                for j in 1..21 {
                    let cj = c + dc*((j as f32)*0.5);
                    let tj = self.tripoint_time(p, cj, n, atlas);

                    if tj < tmin {
//                        let l = (c - cj).abs();
//                        println!("Moving point {} meters", l);
                        self.points[i] = cj;
                        tmin = tj;
                    }
                }
            }

            // Split long segments, join nearby vertices.
            let mut new_points = vec!();
            // Always push start point
            let mut c = self.points[0];
            new_points.push(c);
            let mut i = 1;

            loop {
                let n = self.points[i];

                if i == len - 1 {
                    // Always push end point
                    new_points.push(n);
                    break;
                }

                let d = (n - c).abs();

                if d > 20.0 {
                    // Long distance. Create intermediate point between this
                    // one and the next.
                    let c2 = (c + n)*0.5;
                    // Check that path exists from current point via
                    // intermediate ptoint to next point.
                    if self.tripoint_time(c, c2, n, atlas).is_finite() {
                        new_points.push(c2);
                        c = c2;
                        continue;
                    }
                }

                if d < 10.0 && i + 1 < len {
                    // Short distance.
                    // Check that path exists from current point to the point
                    // beyond the next one. Then skip the next point.
                    if let Some(_) = Segment::new(c, self.points[i + 1])
                        .time(atlas) {
                        i += 1;
                        continue;
                    }
                }

                // Medium distance (no changes to path). Push point and
                // increment.
                new_points.push(n);
                c = n;
                i += 1;
            }

            let time2 = self.calculate_time(atlas);

            println!("After adjustments: Time {}, points {}", time2,
                     self.points.len());
            if time - time2 < 0.001 {
                break;
            }

            time = time2;

            if time2 != 0.0 && time2.is_finite() {
                self.points = new_points;
            }
            else {
                println!("Path is no longer walkable");
                println!("Old path: {}", self.points.len());
                println!("New path: {}", new_points.len());
            }
        }
    }

    pub fn calculate_time(&self, atlas: &Atlas) -> f32 {
        let mut time = 0.0;

        for i in 0..self.points.len() - 1 {
            if let Some(t) = Segment::new(self.points[i],
                                          self.points[i + 1]).time(atlas) {
                time += t;
            }
            else {
                return f32::INFINITY;
            }
        }

        return time;
    }

    pub fn len(&self) -> f32 {
        let mut l = 0.0;

        for i in 0..self.points.len() - 1 {
            l += Segment::new(self.points[i], self.points[i + 1]).len();
        }

        return l;
    }

    pub fn elevation(&self, atlas: &Atlas) -> f32 {
        let mut h = 0.0;

        // Calculate the accumulated relative elevation along the track. Downhill parts
        // are not counted.
        for i in 0..self.points.len() - 1 {
            h += Segment::new(self.points[i], self.points[i + 1]).height(atlas);
        }

        return h;
    }

    pub fn descent(&self, atlas: &Atlas) -> f32 {
        let mut h = 0.0;

        // Descent is calculated in the same way as height, but in the oposite direction.
        for i in 0..self.points.len() - 1 {
            h += Segment::new(self.points[i + 1], self.points[i]).height(atlas);
        }

        return h;
    }

    pub fn read_gpx(fname: &str) -> Self {
	let file = File::open(fname).unwrap();
	let reader = BufReader::new(file);

	let mut points = vec!();

	let gpx: Gpx = gpx::read(reader).unwrap();
	// Assume first track in file is the one to use.
	let track: &Track = &gpx.tracks[0];

	for wp in &track.segments[0].points {
	    points.push(Coord::from_latlon(wp.point().y(), wp.point().x()));
	}

        Self {
            points: points,
            time: 0.0,
        }
    }

    pub fn write_gpx(&self, fname: &str, name: &str, atlas: &Atlas) {
        let track_segment = TrackSegment {
            points: vec![]
        };
        let track = Track {
            name: Some(name.to_string()),
            comment: None,
            description: None,
            source: None,
            links: vec![],
            type_: None,
            number: None,
            segments: vec![track_segment],
        };
        let mut gpx = Gpx {
            version: GpxVersion::Gpx11,
            creator: None,
            metadata: Some(Metadata {
                name: Some(name.to_string()),
                description: None,
                author: None,
                links: vec![],
                time: None,
                keywords: None,
                copyright: None,
                bounds: None,
            }),
            waypoints: vec![],
            tracks: vec![track],
            routes: vec![],
        };

        // Create file at path
        let gpx_file = File::create(fname).unwrap();
        let buf = BufWriter::new(gpx_file);

        // Add track point
        for p in &self.points {
            // Coordinates path are stored in UTM33
            // Coordinates in the gpx file are stored in the WGS-84 system.
	    /*
            let (lat, long) = wsg84_utm_to_lat_lon(
            p.e as f64, p.n as f64, 33, 'W').unwrap();
	     */
	    let (lat, long) = p.latlon();
            let mut wp = Waypoint::new(Point::new(long, lat));
            wp.elevation = Some(atlas.lookup(&p).unwrap().into());
            gpx.tracks[0].segments[0].points.push(wp);
        }

        // Write to file
        gpx::write(&gpx, buf).unwrap();
    }

    pub fn print_summary(&self, atlas: &Atlas) {
        println!("Path: {}", self);
        println!("Length: {}m", self.len());
        let time = self.calculate_time(atlas) as usize;
        match time {
            t if t >= 3600 => {
                println!("Time: {} hr {} min {} sec",
                         t/3600, (t%3600)/60, t%60);
            },
            t if t >= 60 => {
                println!("Time: {} min {} sec", t/60, t%60);
            },
            t => {
                println!("Time: {} sec", t);
            },
        }
        println!("Total elevation: {}m", self.elevation(&atlas));
        println!("Total descent: {}m", self.descent(&atlas));
    }
}

impl<'a> IntoIterator for &'a Path {
    type Item = &'a Coord;

    type IntoIter = Iter<'a, Coord>;

    fn into_iter(self) -> Self::IntoIter {
        self.points.iter()
    }
}

impl fmt::Display for Path {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = self.points.len();
        formatter.write_fmt(format_args!("{} -> {} ({} pts)",
                                         self.points[0], self.points[c - 1],
                                         c))
    }
}
