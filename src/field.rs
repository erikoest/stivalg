use std::fmt;
use hoydedata::Coord;

// Length and width of each field in grid (meter)
pub const FIELD_SIZE: f32 = 1.0;

// Origo (n and e coordinate) of
pub const E_ORIGO: f32 = 0.0;
pub const N_ORIGO: f32 = 0.0;

#[derive(Copy, Debug, Clone, PartialEq)]
pub struct Field {
    pub x: u32,
    pub y: u32,
}

impl Field {
    pub fn new(x: u32, y: u32) -> Self {
        Self {
            x: x,
            y: y,
        }
    }

    // Determine crossing point into the next field. Input is a (reference
    // coordinate) and b (end coordinate). Return crossing point and the
    // next field. If b is in this field, return this field and the point b.
    pub fn crossing(&self, a: &Coord, b: &Coord) -> Option<(Coord, Field)> {
        let s = (self.y as f32)*FIELD_SIZE + E_ORIGO;
        let n = (self.y as f32 + 1.0)*FIELD_SIZE + E_ORIGO;
        let w = (self.x as f32)*FIELD_SIZE + E_ORIGO;
        let e = (self.x as f32 + 1.0)*FIELD_SIZE + E_ORIGO;

        let (n2, e2);
        let (x2, y2);

        if b.n > n {
            if b.e < w {
                // North west
                let n_tmp = a.n + (b.n - a.n)*(a.e - w)/(a.e - b.e);
                if n_tmp < n {
                    e2 = w;
                    n2 = n_tmp;
                    x2 = self.x - 1;
                    y2 = self.y;
                }
                else {
                    e2 = a.e + (b.e - a.e)*(n - a.n)/(b.n - a.n);
                    n2 = n;
                    x2 = self.x;
                    y2 = self.y + 1;
                }
            }
            else if b.e > e {
                // North east
                let n_tmp = a.n + (b.n - a.n)*(e - a.e)/(b.e - a.e);
                if n_tmp < n {
                    n2 = n_tmp;
                    e2 = e;
                    x2 = self.x + 1;
                    y2 = self.y;
                }
                else {
                    n2 = n;
                    e2 = a.e + (b.e - a.e)*(n - a.n)/(b.n - a.n);
                    x2 = self.x;
                    y2 = self.y + 1;
                }
            }
            else {
                // North
                n2 = n;
                e2 = a.e + (b.e - a.e)*(n - a.n)/(b.n - a.n);
                x2 = self.x;
                y2 = self.y + 1;
            }
        }
        else if b.n < s {
            if b.e < w {
                // South west
                let n_tmp = a.n + (b.n - a.n)*(a.e - w)/(a.e - b.e);
                if n_tmp > s {
                    e2 = w;
                    n2 = n_tmp;
                    x2 = self.x - 1;
                    y2 = self.y;
                }
                else {
                    e2 = a.e + (b.e - a.e)*(a.n - s)/(a.n - b.n);
                    n2 = s;
                    x2 = self.x;
                    y2 = self.y - 1;
                }
            }
            else if b.e > e {
                // South east
                let n_tmp = a.n + (b.n - a.n)*(e - a.e)/(b.e - a.e);
                if n_tmp > s {
                    n2 = n_tmp;
                    e2 = e;
                    x2 = self.x + 1;
                    y2 = self.y - 1;
                }
                else {
                    n2 = s;
                    e2 = a.e + (b.e - a.e)*(a.n - s)/(a.n - b.n);
                    x2 = self.x + 1;
                    y2 = self.y - 1;
                }
            }
            else {
                // South
                e2 = a.e + (b.e - a.e)*(a.n - s)/(a.n - b.n);
                n2 = s;
                x2 = self.x;
                y2 = self.y - 1;
            }
        }
        else {
            if b.e < w {
                // West
                e2 = w;
                n2 = a.n + (b.n - a.n)*(a.e - w)/(a.e - b.e);
                x2 = self.x - 1;
                y2 = self.y;
            }
            else if b.e > e {
                // East
                e2 = e;
                n2 = a.n + (b.n - a.n)*(e - a.e)/(b.e - a.e);
                x2 = self.x + 1;
                y2 = self.y;
            }
            else {
                return None;
            }
        }

        return Some((
            Coord::new(e2, n2),
            Field::new(x2, y2),
        ));
    }
}

impl From<Coord> for Field {
    fn from(c: Coord) -> Self {
        Field::new(
            ((c.e - E_ORIGO)/FIELD_SIZE) as u32,
            ((c.n - N_ORIGO)/FIELD_SIZE) as u32,
        )
    }
}

impl Into<Coord> for Field {
    fn into(self) -> Coord {
        Coord::new(
            (self.x as f32)*FIELD_SIZE + E_ORIGO,
            (self.y as f32)*FIELD_SIZE + N_ORIGO
        )
    }
}

impl fmt::Display for Field {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_fmt(format_args!("field({}, {})", self.x, self.y))
    }
}
