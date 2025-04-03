use crate::barrier::Barrier;
use crate::params::Params;
use crate::path::{Segment, Path};

use hoydedata::{Atlas, Coord};
use std::cmp::max;
use std::collections::HashMap;

pub struct Graph {
    a: Coord,
    b: Coord,
    o: Coord,
    f1: Coord,
    f2: Coord,
    major: f32,
    gs_pass1: f32,
    gs_pass2: f32,
    g_pass1: usize,
    g_pass2: usize,
    barriers: Vec<Barrier>,
    cmap: HashMap<usize, usize>,
    v: usize,
    edges: Vec<(usize, usize, f32)>,
    nodes: Vec<Coord>,
}

impl Graph {
    pub fn new(a: Coord, b: Coord, params: &Params) -> Self {
        // Center
        let o = (a + b)*0.5;
        // Radius
        let r = (a - o).abs();
        // Ellipse length
        let major = r*params.covering_length;
        // Ellipse width
        let minor = r*params.covering_width;
        // Focal points
        let f = (major*major - minor*minor).sqrt();
        let f1 = (a - o)*(f/major) + o;
        let f2 = (b - o)*(f/major) + o;

        // Grid width
        let g_pass1 = ((major/params.grid_size_pass1) as usize)*2 + 1;
        let g_pass2 = ((major/params.grid_size_pass2) as usize)*2 + 1;

        Self {
            a: a,
            b: b,
            o: o,
            f1: f1,
            f2: f2,
            major: major,
            gs_pass1: params.grid_size_pass1,
            gs_pass2: params.grid_size_pass2,
            g_pass1: g_pass1,
            g_pass2: g_pass2,
            barriers: params.barriers.clone(),
            cmap: HashMap::new(),
            v: 0,
            edges: vec!(),
            nodes: vec!(),
        }
    }

    pub fn num_nodes(&self) -> usize {
        return self.nodes.len();
    }

    pub fn num_edges(&self) -> usize {
        return self.edges.len();
    }

    fn node_exists(&self, x: usize, y: usize) -> bool {
        let hash_key = (x + y) * (x + y + 1) / 2 + x;
        return self.cmap.contains_key(&hash_key);
    }

    // Get a coordinate based on grid coordinates. The coordinate is returned
    // together with its vertex number.
    fn insert_node_from_grid_units(&mut self, gs: f32, g: usize, x: usize,
                                   y: usize, check_area: bool)
                                   -> Option<(Coord, usize)> {
        let e = (x as f32)*gs + self.o.e - (((g - 1)/2) as f32)*gs;
        let n = (y as f32)*gs + self.o.n - (((g - 1)/2) as f32)*gs;
        let c = Coord::new(e, n);

        if check_area {
            // Coordinates must be within the area of an ellipse with focal
            // points f1 and f2
            if (c - self.f1).abs() + (c - self.f2).abs() > 2.0*self.major {
                // Coordinate is not within the area. Return nothing.
                return None;
            }
        }

        // Use cantors pairing function for the hash key
        let hash_key = (x + y) * (x + y + 1) / 2 + x;
        if let Some(v) = self.cmap.get(&hash_key) {
            // Coordinate already exists. Return the old vertex number
            return Some((c, *v));
        }
        else {
            // New coordinate. Get a new vertex number
            let v = self.v;
            self.v += 1;
            self.cmap.insert(hash_key, v);
            self.nodes.push(c.clone());
            return Some((c, v));
        }
    }

    fn insert_node_from_coord(&mut self, c: Coord) -> (Coord, usize) {
        let v = self.v;
        let n = (c.clone(), v);
        self.nodes.push(c);
        self.v += 1;
        return n;
    }

    fn connect(&mut self, opt_c1: Option<(Coord, usize)>,
               opt_c2: Option<(Coord, usize)>, atlas: &Atlas) {
        if let Some((c1, cn1)) = opt_c1 {
            if let Some((c2, cn2)) = opt_c2 {
                for b in &self.barriers {
                    if b.is_crossing(&c1, &c2) {
                        return;
                    }
                }

                if let Some(time1) = Segment::new(c1, c2).time(atlas) {
                    self.edges.push((cn1, cn2, time1));
                }
                if let Some(time2) = Segment::new(c2, c1).time(atlas) {
                    self.edges.push((cn2, cn1, time2));
                }
            }
        }
    }

    // Dijkstra's algorithm for finding the shortest path from first to
    // last node.
    pub fn shortest_path(&mut self) -> Option<Path> {
        // Build graph of lists of nodes and adjacent nodes.
        let start = 0;
        let end = self.v - 1;
        let mut times: Vec<f32> = vec!();
        let mut adj: Vec<[(usize, f32); 10]> = vec!();
        let mut adj_count: Vec<usize> = vec!();
        let mut prev: Vec<Option<usize>> = vec!();
        let mut visited: Vec<bool> = vec!();

        let nn = self.num_nodes();

        for _ in 0..nn {
            times.push(f32::INFINITY);
            adj.push([(0, f32::INFINITY); 10]);
            adj_count.push(0);
            prev.push(None);
            visited.push(false);
        }

        // Populate adjacency list.
        for (n1, n2, t) in &self.edges {
            adj[*n1][adj_count[*n1]] = (*n2, *t);
            adj_count[*n1] += 1;
        }

        // We may change this to a priority queue with better performance.
        let mut queue = HashMap::new();
        queue.insert(start, 1);
        times[start] = 0.0;
        visited[start] = true;

        loop {
            // Find minimum node in queue
            let mut t_min = f32::INFINITY;
            let mut n_min = 0;
            for i in queue.keys() {
                let t = times[*i];
                if t < t_min {
                    t_min = t;
                    n_min = *i;
                }
            }

            if t_min == f32::INFINITY {
                break;
            }

            queue.remove(&n_min);
            if n_min == end {
                break;
            }

            // Look at each neighbour to the minimum node
            for ac in 0..adj_count[n_min] {
                let (n_adj, t_edge) = adj[n_min][ac];
                if !visited[n_adj] {
                    queue.insert(n_adj, 1);
                }
                let t_new = t_min + t_edge;
                if t_new < times[n_adj] {
                    times[n_adj] = t_new;
                    prev[n_adj] = Some(n_min);
                }
            }

            visited[n_min] = true;
        }

        if times[end] == f32::INFINITY {
            return None;
        }

        let mut p = end;
        let mut reverse = vec!();
        loop {
            reverse.push(self.nodes[p]);
            if let Some(prev) = prev[p] {
                p = prev;
            }
            else {
                break;
            }
        }

        let mut p = Path::new();
        while let Some(c) = reverse.pop() {
            p.push(c);
        }

        return Some(p);
    }

    fn grid_units_for_node(&self, c: &Coord, gs: f32, g: usize)
                           -> (usize, usize) {
        let x = ((c.e - self.o.e)/gs + ((g - 1)/2) as f32) as usize;
        let y = ((c.n - self.o.n)/gs + ((g - 1)/2) as f32) as usize;

        return (x, y);
    }

    fn connect_end_node(&mut self, c: Option<(Coord, usize)>, gs: f32,
                        g: usize, atlas: &Atlas) {
        if let Some((c1, _)) = c {
            let (x, y) = self.grid_units_for_node(&c1, gs, g);
            let s1 = self.insert_node_from_grid_units(gs, g, x, y, false);
            let s2 = self.insert_node_from_grid_units(gs, g, x + 1, y, false);
            let s3 = self.insert_node_from_grid_units(gs, g, x, y + 1, false);
            let s4 = self.insert_node_from_grid_units(gs, g, x + 1, y + 1,
                                                      false);
            self.connect(c, s1, atlas);
            self.connect(c, s2, atlas);
            self.connect(c, s3, atlas);
            self.connect(c, s4, atlas);
        }
    }

    pub fn add_pass2_node(&mut self, x: usize, y: usize, atlas: &Atlas) {
        // Return if point has already been added
        if self.node_exists(x, y) {
            return;
        }

        let c = self.insert_node_from_grid_units(
            self.gs_pass2, self.g_pass2, x, y, false);

        for (xn, yn) in [(x - 1, y - 1), (x, y - 1), (x + 1, y - 1),
                         (x - 1, y), (x + 1, y), (x - 1, y + 1),
                         (x, y + 1), (x + 1, y + 1)] {
            if !self.node_exists(xn, yn) {
                continue;
            }

            let cn = self.insert_node_from_grid_units(
                self.gs_pass2, self.g_pass2, xn, yn, false);

            self.connect(c, cn, atlas);
        }
    }

    pub fn add_pass1_node(&mut self, x: usize, y: usize)
                          -> Option<(Coord, usize)> {
        return self.insert_node_from_grid_units(
            self.gs_pass1, self.g_pass1, x, y, true);
    }

    // Build finely grained a graph for the area around a given path. The area
    // is determined by dragging a square along the path.
    pub fn build_graph_from_path(&mut self, path: &Path, atlas: &Atlas) {
        // Finely grained grid size
        let gs = self.gs_pass2;
        // Number of grid points within area diameter
        let g = self.g_pass2;
        // Square size in grid units
        let ss = (self.gs_pass2/gs) as usize;

        // Create start node
        let a = Some(self.insert_node_from_coord(self.a));

        let mut last: Option<Coord> = None;
        // Create intermediate nodes in area along the path
        for c1 in path {
            if let Some(c0) = last {
                let x0 = ((c0.e - self.o.e)/gs + ((g - 1)/2) as f32) as usize;
                let y0 = ((c0.n - self.o.n)/gs + ((g - 1)/2) as f32) as usize;
                let x1 = ((c1.e - self.o.e)/gs + ((g - 1)/2) as f32) as usize;
                let y1 = ((c1.n - self.o.n)/gs + ((g - 1)/2) as f32) as usize;

                let clen = max(if x1 > x0 { x1 - x0 } else { x0 - x1 },
                               if y1 > y0 { y1 - y0 } else { y0 - y1 });
                if clen == 0 {
                    continue;
                }

                for i in 0..clen + 1 {
                    let xn = if x1 > x0 {
                        (x1 - x0)*i/clen + x0 - ss/2
                    }
                    else {
                        x0 - (x0 - x1)*i/clen - ss/2
                    };

                    let yn = if y1 > y0 {
                        (y1 - y0)*i/clen + y0 - ss/2
                    }
                    else {
                        y0 - (y0 - y1)*i/clen - ss/2
                    };

                    for i in 0..ss {
                        self.add_pass2_node(xn + i, yn, atlas);
                        self.add_pass2_node(xn + i + 1, yn + ss, atlas);
                        self.add_pass2_node(xn, yn + i + 1, atlas);
                        self.add_pass2_node(xn + ss, yn + i, atlas);
                    }
                }
            }

            last.replace(c1.clone());
        }

        // Connect start node to graph
        self.connect_end_node(a, gs, g, atlas);

        // Create end node and connect it to graph
        let b = Some(self.insert_node_from_coord(self.b));
        self.connect_end_node(b, gs, g, atlas);
    }

    // Build a coarsely grained graph from the area defined by an ellipse
    // overlapping the start and end points.
    pub fn build_graph_from_end_points(&mut self, atlas: &Atlas) {
        let g = self.g_pass1;

        // Create start node
        let a = Some(self.insert_node_from_coord(self.a));

        // Create intermediate candidate nodes
        for x in 0..g {
            for y in 0..g {
                let c1 = self.add_pass1_node(x, y);
                let c2 = self.add_pass1_node(x + 1, y);
                let c3 = self.add_pass1_node(x, y + 1);
                let c4 = self.add_pass1_node(x + 1, y + 1);

                self.connect(c1, c2, atlas);
                self.connect(c1, c3, atlas);
                self.connect(c1, c4, atlas);
                self.connect(c2, c3, atlas);
            }
        }

        // Connect start node to graph
        self.connect_end_node(a, self.gs_pass1, g, atlas);

        // Create end node and connect it to graph
        let b = Some(self.insert_node_from_coord(self.b));
        self.connect_end_node(b, self.gs_pass1, g, atlas);
    }
}
