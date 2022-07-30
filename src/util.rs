use std::cmp;

#[derive(Copy, Clone, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Copy, Clone, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Rect {
    pub fn intersection(&self, other: Rect) -> Option<Rect> {
        let ix = cmp::max(self.x, other.x);
        let iy = cmp::max(self.y, other.y);
        let iw = cmp::min(self.x + self.w, other.x + other.w) - ix;
        let ih = cmp::min(self.y + self.h, other.y + other.h) - iy;

        if iw > 0 && ih > 0 {
            Some(Rect {
                x: ix,
                y: iy,
                w: iw,
                h: ih,
            })
        } else {
            None
        }
    }

    pub fn contains(&self, pos: Point) -> bool {
        pos.x >= self.x && pos.x < self.x + self.w && pos.y >= self.y && pos.y < self.y + self.h
    }
}

pub fn parse_int<T: num_traits::Num>(string: &str) -> Result<T, T::FromStrRadixErr> {
    if string.len() < 2 {
        return T::from_str_radix(string, 10);
    }
    match &string[..2] {
        "0x" | "0X" => T::from_str_radix(&string[2..], 16),
        "0o" | "0O" => T::from_str_radix(&string[2..], 8),
        "0b" | "0B" => T::from_str_radix(&string[2..], 2),
        _ => T::from_str_radix(string, 10),
    }
}
