// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process;
use std::time;

use getopts::Options;
use image::GenericImage;
use image::Pixel;
use image::Rgba;
use image::RgbaImage;
use x11::xlib;

mod util;
mod xwrap;
use crate::xwrap::Display;

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
    opts.optopt("f", "format", "Output format", "png/pam");
    opts.optflag(
        "s",
        "single-screen",
        "Capture the screen determined by the cursor location",
    );
    opts.optflag("h", "help", "Print help and exit");
    opts.optflag("v", "version", "Print version and exit");

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

    if matches.opt_present("v") {
        eprintln!(
            "shotgun {}",
            option_env!("GIT_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
        );
        return 0;
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
        Some(s) => match util::parse_int::<xlib::Window>(&s) {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Window ID is not a valid integer");
                eprintln!("Accepted values are decimal, hex (0x*), octal (0o*) and binary (0b*)");
                return 1;
            }
        },
        None => root,
    };

    let output_ext = matches
        .opt_str("f")
        .unwrap_or("png".to_string())
        .to_lowercase();
    let output_format = match output_ext.as_ref() {
        "png" => image::ImageOutputFormat::Png,
        "pam" => image::ImageOutputFormat::Pnm(image::pnm::PNMSubtype::ArbitraryMap),
        _ => {
            eprintln!("Invalid image format specified");
            return 1;
        }
    };

    let window_rect = display.get_window_rect(window);

    if matches.opt_present("s") {
        if matches.opt_present("g") {
            eprintln!("Cannot use -g and -s at the same time");
            return 1;
        }
        if matches.opt_present("i") {
            eprintln!("Cannot use -i and -s at the same time");
            return 1;
        }
    }

    let mut sel = match matches.opt_str("g") {
        Some(s) => match xwrap::parse_geometry(CString::new(s).expect("Failed to convert CString"))
            .intersection(window_rect)
        {
            Some(sel) => util::Rect {
                // Selection is relative to the root window (whole screen)
                x: sel.x - window_rect.x,
                y: sel.y - window_rect.y,
                w: sel.w,
                h: sel.h,
            },
            None => {
                eprintln!("Invalid geometry");
                return 1;
            }
        },
        None => util::Rect {
            x: 0,
            y: 0,
            w: window_rect.w,
            h: window_rect.h,
        },
    };

    let screen_rects: Vec<util::Rect> = match display.get_screen_rects(root) {
        Some(r) => r.collect(),
        None => {
            eprintln!("Failed to get screen rects");
            return 1;
        }
    };

    if matches.opt_present("s") {
        let cursor = match display.get_cursor_position(root) {
            Some(c) => c,
            None => {
                eprintln!("Failed to get cursor position");
                return 1;
            }
        };

        // Find the screen that the cursor is on
        sel = match screen_rects.iter().find(|r| r.contains(cursor)) {
            Some(r) => *r,
            None => {
                eprintln!("Failed to find screen containing cursor");
                return 1;
            }
        }
    }

    let image = match display.get_image(window, sel) {
        Some(i) => i,
        None => {
            eprintln!("Failed to get image from X");
            return 1;
        }
    };

    let mut image = match image.into_image_buffer() {
        Some(i) => image::DynamicImage::ImageRgba8(i),
        None => {
            eprintln!(
                "Failed to convert captured framebuffer, only 24/32 \
                      bit (A)RGB8 is supported"
            );
            return 1;
        }
    };

    // When capturing the root window, attempt to mask the off-screen areas
    if window == root {
        let screens: Vec<util::Rect> = screen_rects
            .iter()
            .filter_map(|s| s.intersection(sel))
            .collect();

        // No point in masking if we're only capturing one screen
        if screens.len() > 1 {
            let mut masked =
                RgbaImage::from_pixel(sel.w as u32, sel.h as u32, Rgba::from_channels(0, 0, 0, 0));

            for screen in screens {
                // Subimage is relative to the captured area
                let sub = util::Rect {
                    x: screen.x - sel.x,
                    y: screen.y - sel.y,
                    w: screen.w,
                    h: screen.h,
                };

                let mut sub_src =
                    image.sub_image(sub.x as u32, sub.y as u32, sub.w as u32, sub.h as u32);
                masked
                    .copy_from(&mut sub_src, sub.x as u32, sub.y as u32)
                    .expect("Failed to copy sub-image");
            }

            image = image::DynamicImage::ImageRgba8(masked);
        }
    }

    let ts_path = {
        let now = match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
            Ok(n) => n.as_secs(),
            Err(_) => 0,
        };
        format!("{}.{}", now, output_ext)
    };
    let path = match matches.free.get(0) {
        Some(p) => p,
        None => {
            eprintln!("No output specified, defaulting to {}", ts_path);
            ts_path.as_str()
        }
    };

    if path == "-" {
        image
            .write_to(&mut io::stdout(), output_format)
            .expect("Writing to stdout failed");
    } else {
        match File::create(&Path::new(&path)) {
            Ok(mut f) => image
                .write_to(&mut f, output_format)
                .expect("Writing to file failed"),
            Err(e) => {
                eprintln!("Failed to create {}: {}", path, e);
                return 1;
            }
        }
    }

    0
}

fn main() {
    process::exit(run());
}
