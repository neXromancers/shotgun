use std::path::Path;

extern crate image;
extern crate libc;
extern crate x11;
use x11::xlib;

mod xwrap;
use xwrap::Display;

fn main() {
    let display = Display::open(None).unwrap();
    let root = display.get_default_root();
    let attrs = display.get_window_attributes(root);
    let image = display.get_image(root, 0, 0,
                                  attrs.width as libc::c_uint, attrs.height as libc::c_uint,
                                  xwrap::ALL_PLANES, xlib::ZPixmap).unwrap();

    let path = Path::new("shotgun.png");
    // FIXME handle errors
    if let Ok(buf) = image.into_image_buffer() {
        buf.save(path).unwrap();
    }
}
