use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::ptr;

extern crate libc;
extern crate png;
use png::HasParameters;
extern crate x11;
use x11::xlib;

mod xwrap;
use xwrap::Display;

fn main() {
    let display = Display::open(ptr::null()).unwrap();
    let root = display.get_default_root();
    let attrs = display.get_window_attributes(root);
    let image = display.get_image(root, 0, 0,
                                  attrs.width as libc::c_uint, attrs.height as libc::c_uint,
                                  xwrap::ALL_PLANES, xlib::ZPixmap).unwrap();

    let path = Path::new("shotgun.png");
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, attrs.width as u32, attrs.height as u32);
    encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(image.get_data()).unwrap();
}
