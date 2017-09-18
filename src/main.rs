use std::cmp;
use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process;

extern crate getopts;
use getopts::Options;
extern crate image;
use image::GenericImage;
use image::Pixel;
use image::RgbaImage;
use image::Rgba;
extern crate isatty;
extern crate libc;
extern crate time;
extern crate x11;
use x11::xlib;

mod xwrap;
use xwrap::Display;

fn usage(progname: &str, opts: getopts::Options) {
    let brief = format!("Usage: {} [options] [file]", progname);
    eprint!("{}", opts.usage(&brief));
}

fn run() -> i32 {
    let args: Vec<String> = env::args().collect();
    let progname = args[0].clone();

    let mut opts = Options::new();
    opts.optopt("i", "id", "Window to capture", "ID");
    opts.optopt("g", "geometry", "Area to capture", "WxH+X+Y");
    opts.optflag("h", "help", "Print help and exit");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            usage(&progname, opts);
            return 1;
        }
    };

    if matches.opt_present("h") {
        usage(&progname, opts);
        return 0;
    }

    // One loose argument allowed (file name)
    if matches.free.len() > 1 {
        eprintln!("Too many arguments");
        usage(&progname, opts);
        return 1;
    }

    let display = match Display::open(None) {
        Some(d) => d,
        None => {
            eprintln!("Failed to open display");
            return 1;
        }
    };
    let root = display.get_default_root();

    let window = match matches.opt_str("i") {
        Some(s) => match s.parse::<xlib::Window>() {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Window ID is not a valid integer");
                return 1;
            },
        },
        None => root,
    };

    let attrs = display.get_window_attributes(window);
    let (w, h, x, y) = match matches.opt_str("g") {
        Some(s) => xwrap::parse_geometry(CString::new(s).expect("Failed to convert CString")),
        None => {
            (attrs.width as libc::c_uint, attrs.height as libc::c_uint, 0, 0)
        },
    };
    if w <= 0 || h <= 0 || x < 0 || y < 0
        || x + w as libc::c_int > attrs.width || y + h as libc::c_int > attrs.height {
        eprintln!("Invalid geometry");
        return 1;
    }

    let image = match display.get_image(window, x, y, w, h, xwrap::ALL_PLANES, xlib::ZPixmap) {
        Some(i) => i,
        None => {
            eprintln!("Failed to get image from X");
            return 1;
        },
    };

    let mut image = match image.into_image_buffer() {
        Some(i) => image::ImageRgba8(i),
        None => {
            eprintln!("Failed to convert captured framebuffer, only 24/32 \
                      bit (A)RGB8 is supported");
            return 1;
        }
    };

    if window == root {
        match display.get_screen_rects(root) {
            Some(screens) => {
                let mut masked = RgbaImage::from_pixel(w, h, Rgba::from_channels(0, 0, 0, 0));

                for (sw, sh, sx, sy) in screens {
                    // Clamp the area to copy
                    let sub_x = cmp::max(x, sx);
                    let sub_y = cmp::max(y, sy);
                    let sub_w = cmp::min(x + w as i32, sx + sw) - sub_x;
                    let sub_h = cmp::min(y + h as i32, sy + sh) - sub_y;

                    // Recalculate x and y relative to the captured area
                    let sub_x = sub_x - x;
                    let sub_y = sub_y - y;

                    if sub_w > 0 && sub_h > 0 {
                        let mut sub_src = image.sub_image(sub_x as u32, sub_y as u32,
                                                          sub_w as u32, sub_h as u32);
                        masked.copy_from(&mut sub_src, sub_x as u32, sub_y as u32);
                    }
                }

                image = image::ImageRgba8(masked);
            },
            None => {
                eprintln!("Failed to enumerate screens, not masking");
            },
        }
    }

    let ts_path = format!("{}.png", time::get_time().sec);
    let path = match matches.free.get(0) {
        Some(p) => p,
        None => if !isatty::stdout_isatty() {
            "-"
        } else {
            ts_path.as_str()
        },
    };

    if path == "-" {
        image.save(&mut io::stdout(), image::PNG).expect("Writing to stdout failed");
    } else {
        match File::create(&Path::new(&path)) {
            Ok(mut f) => image.save(&mut f, image::PNG).expect("Writing to file failed"),
            Err(e) => {
                eprintln!("Failed to create {}: {}", path, e);
                return 1
            },
        }
    }

    0
}

fn main() {
    process::exit(run());
}
