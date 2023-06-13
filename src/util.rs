use std::cmp;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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

use image::EncodableLayout;
pub fn write_image_buffer_with_encoder<P, Container>(
    image: &image::ImageBuffer<P, Container>,
    encoder: impl image::ImageEncoder,
) -> image::ImageResult<()>
where
    P: image::PixelWithColorType,
    [P::Subpixel]: image::EncodableLayout,
    Container: core::ops::Deref<Target = [P::Subpixel]>,
{
    encoder.write_image(
        image.as_raw().as_bytes(),
        image.width(),
        image.height(),
        P::COLOR_TYPE,
    )
}

mod parse_geometry {
    use crate::util;

    use nom::bytes::complete as bytes;
    use nom::character::complete as chr;
    use nom::combinator as comb;
    use nom::sequence as seq;

    fn equal_sign(i: &str) -> nom::IResult<&str, char> {
        chr::char('=')(i)
    }

    fn x_sign(i: &str) -> nom::IResult<&str, char> {
        chr::one_of("xX")(i)
    }

    fn integer(i: &str) -> nom::IResult<&str, i32> {
        comb::map_res(
            // Limit to 5 digits - X11 uses i16 and u16 for position and size,
            // and this fits comfortably into our i32 Rects.
            bytes::take_while_m_n(1, 5, |c: char| c.is_ascii_digit()),
            str::parse,
        )(i)
    }

    fn sign(i: &str) -> nom::IResult<&str, i32> {
        comb::map(chr::one_of("-+"), |s| if s == '-' { -1 } else { 1 })(i)
    }

    fn signed_integer(i: &str) -> nom::IResult<&str, i32> {
        comb::map(seq::pair(sign, integer), |(s, m)| s * m)(i)
    }

    /// Parse a string of the form `=<width>x<height>{+-}<xoffset>{+-}<yoffset>` into a [`util::Rect`].
    pub fn parse_geometry(g: &str) -> Option<util::Rect> {
        let (remainder, (_, w, _, h, x, y)) = seq::tuple((
            comb::opt(equal_sign),
            integer,
            x_sign,
            integer,
            signed_integer,
            signed_integer,
        ))(g)
        .ok()?;

        if !remainder.is_empty() {
            return None;
        }

        Some(util::Rect { w, h, x, y })
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[test]
        fn test_parse_geometry() {
            let res = Some(util::Rect {
                w: 80,
                h: 24,
                x: 300,
                y: -49,
            });
            assert_eq!(parse_geometry("=80x24+300-49"), res);
            assert_eq!(parse_geometry("80x24+300-49"), res);
        }
    }
}

pub use parse_geometry::parse_geometry;
