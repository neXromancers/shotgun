// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::{
    convert::TryFrom,
    env,
    ffi::{CString, OsStr, OsString},
    fmt,
    fs::File,
    io,
    path::{Path, PathBuf},
    process, time,
};

use image::{GenericImage, Pixel, Rgba, RgbaImage};
use x11::xlib;

mod util;
mod xwrap;
use crate::xwrap::Display;

fn usage(progname: impl fmt::Display, options: &getopts::Options) {
    let brief = format!("Usage: {} [options] [file]", progname);
    eprint!("{}", options.usage(&brief));
}

struct Opts {
    format: Option<String>,
    geometry: Option<String>,
    id: Option<String>,
    path: Option<OsString>,
    #[allow(dead_code)]
    verbosity: isize,
}

impl Opts {
    fn init_options(options: &mut getopts::Options) -> &mut getopts::Options {
        options
            .optopt("i", "id", "Window to capture", "ID")
            .optopt("g", "geometry", "Area to capture", "WxH+X+Y")
            .optopt("f", "format", "Output format", "png/pam")
            .optflagmulti("q", "quiet", "Decrease informational output")
            .optflagmulti("v", "verbose", "Increase informational output")
            .optflag("h", "help", "Print help and exit")
            .optflag("V", "version", "Print version and exit")
    }

    fn parse_options<S: AsRef<OsStr>>(args: &[S], options: &getopts::Options) -> Result<Self, i32> {
        let progname = Path::new(args[0].as_ref()).display();
        let matches = options.parse(args.as_ref()).map_err(|e| {
            eprintln!("{}", e.to_string());
            usage(&progname, options);
            1
        })?;

        if matches.opt_present("h") {
            usage(&progname, options);
            return Err(0);
        }

        if matches.opt_present("V") {
            eprintln!(
                "shotgun {}",
                option_env!("GIT_VERSION").unwrap_or_else(|| env!("CARGO_PKG_VERSION"))
            );
            return Err(0);
        }

        // One loose argument allowed (file name)
        if matches.free.len() > 1 {
            eprintln!("Too many arguments");
            usage(&progname, &options);
            return Err(1);
        }

        Ok(Self {
            format: matches.opt_str("f"),
            geometry: matches.opt_str("g"),
            id: matches.opt_str("i"),
            path: matches.free.get(0).map(OsString::from),
            verbosity: {
                use std::isize::MAX;
                let verbose = isize::try_from(matches.opt_count("v")).unwrap_or(MAX);
                let quiet = isize::try_from(matches.opt_count("q")).unwrap_or(MAX);
                verbose.saturating_sub(quiet)
            },
        })
    }

    fn new<S: AsRef<OsStr>>(args: &[S]) -> Result<Self, i32> {
        Self::parse_options(args, Self::init_options(&mut getopts::Options::new()))
    }
}

fn run() -> i32 {
    let args: Vec<OsString> = env::args_os().collect();

    let opts = match Opts::new(&args[1..]) {
        Ok(v) => v,
        Err(status) => return status,
    };

    let display = match Display::open(None) {
        Some(d) => d,
        None => {
            eprintln!("Failed to open display");
            return 1;
        }
    };
    let root = display.get_default_root();

    let window: xlib::Window = match opts.id {
        Some(s) => match util::parse_int(&s) {
            Ok(r) => r,
            Err(_) => {
                eprintln!("Window ID is not a valid integer");
                eprintln!("Accepted values are decimal, hex (0x*), octal (0o*) and binary (0b*)");
                return 1;
            }
        },
        None => root,
    };

    let output_ext = opts.format.unwrap_or("png".to_string()).to_lowercase();
    let output_format = match output_ext.as_ref() {
        "png" => image::ImageOutputFormat::Png,
        "pam" => image::ImageOutputFormat::Pnm(image::pnm::PNMSubtype::ArbitraryMap),
        _ => {
            eprintln!("Invalid image format specified");
            return 1;
        }
    };

    let window_rect = display.get_window_rect(window);
    let sel = match opts.geometry {
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

    let image = match display.get_image(window, sel, xwrap::ALL_PLANES, xlib::ZPixmap) {
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
        match display.get_screen_rects(root) {
            Some(screens) => {
                let screens: Vec<util::Rect> =
                    screens.filter_map(|s| s.intersection(sel)).collect();

                // No point in masking if we're only capturing one screen
                if screens.len() > 1 {
                    let mut masked = RgbaImage::from_pixel(
                        sel.w as u32,
                        sel.h as u32,
                        Rgba::from_channels(0, 0, 0, 0),
                    );

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
            None => {
                eprintln!("Failed to enumerate screens, not masking");
            }
        }
    }

    let path: OsString = opts.path.unwrap_or_else(|| {
        let now = match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
            Ok(n) => n.as_secs(),
            Err(_) => 0,
        };
        let ts_path = format!("{}.{}", now, output_ext);
        eprintln!("No output specified, defaulting to {}", ts_path);
        OsString::from(ts_path)
    });

    if path == "-" {
        image
            .write_to(&mut io::stdout(), output_format)
            .expect("Writing to stdout failed");
    } else {
        let path = PathBuf::from(path);
        match File::create(&path) {
            Ok(mut f) => image
                .write_to(&mut f, output_format)
                .expect("Writing to file failed"),
            Err(e) => {
                eprintln!("Failed to create {}: {}", path.display(), e);
                return 1;
            }
        }
    }

    0
}

fn main() {
    process::exit(run());
}
