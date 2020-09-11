// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::{
    convert::TryFrom,
    env,
    ffi::{CString, OsStr, OsString},
    fmt,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    process, time,
};

use image::{GenericImage, ImageOutputFormat, Pixel, Rgba, RgbaImage};
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
    geometry: Option<CString>,
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
            geometry: matches
                .opt_str("g")
                .map(|s| {
                    CString::new(s).map_err(|_e| {
                        eprintln!("Failed to convert geometry to CString (contains NUL?)");
                        1
                    })
                })
                .transpose()?,
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

enum ParsedPath<P> {
    Path(P),
    Stdout,
}

impl<P: std::cmp::PartialEq<&'static str>> From<P> for ParsedPath<P> {
    fn from(path: P) -> Self {
        if path == "-" {
            Self::Stdout
        } else {
            Self::Path(path)
        }
    }
}
impl<P> ParsedPath<P> {
    fn map_path<Q, F>(self, f: F) -> ParsedPath<Q>
    where
        F: FnOnce(P) -> Q,
    {
        match self {
            Self::Path(path) => ParsedPath::Path(f(path)),
            Self::Stdout => ParsedPath::Stdout,
        }
    }
}

struct ParsedOpts {
    geometry: Option<util::Rect>,
    output_ext: String,
    output_format: ImageOutputFormat,
    path: Option<ParsedPath<PathBuf>>,
    #[allow(dead_code)]
    verbosity: isize,
    window: Option<xlib::Window>,
}

impl TryFrom<Opts> for ParsedOpts {
    type Error = i32;
    fn try_from(opts: Opts) -> Result<Self, Self::Error> {
        let verbosity = opts.verbosity;
        let window = opts
            .id
            .map(|s| {
                util::parse_int::<xlib::Window>(&s).map_err(|_| {
                    eprintln!("Window ID is not a valid integer");
                    eprintln!(
                        "Accepted values are decimal, hex (0x*), octal (0o*) and binary (0b*)"
                    );
                    1
                })
            })
            .transpose()?;
        let output_ext = opts
            .format
            .unwrap_or_else(|| String::from("png"))
            .to_lowercase();
        let output_format = match output_ext.as_ref() {
            "png" => Ok(ImageOutputFormat::Png),
            "pam" => Ok(ImageOutputFormat::Pnm(image::pnm::PNMSubtype::ArbitraryMap)),
            _ => {
                eprintln!("Invalid image format specified");
                Err(1)
            }
        }?;
        let geometry = opts.geometry.map(xwrap::parse_geometry);
        let path = opts
            .path
            .map(|p| ParsedPath::from(p).map_path(PathBuf::from));
        Ok(ParsedOpts {
            geometry,
            output_ext,
            output_format,
            path,
            verbosity,
            window,
        })
    }
}

fn timestamp_path(ext: &str) -> PathBuf {
    let now = match time::SystemTime::now().duration_since(time::UNIX_EPOCH) {
        Ok(n) => n.as_secs(),
        Err(_) => 0,
    };
    let ts_path = format!("{}.{}", now, ext);
    PathBuf::from(ts_path)
}

fn run() -> Result<i32, i32> {
    let args: Vec<OsString> = env::args_os().collect();

    let opts = Opts::new(&args[1..])?;

    let parsed_opts = ParsedOpts::try_from(opts)?;

    let display = Display::open(None).ok_or_else(|| {
        eprintln!("Failed to open display");
        1
    })?;
    let root = display.get_default_root();

    let window: xlib::Window = parsed_opts.window.unwrap_or(root);

    let window_rect = display.get_window_rect(window);
    let sel = parsed_opts
        .geometry
        .map(|g| {
            g.intersection(window_rect).ok_or_else(|| {
                eprintln!("Invalid geometry");
                1
            })
        })
        .transpose()?
        .map(|sel| util::Rect {
            // Selection is relative to the root window (whole screen)
            x: sel.x - window_rect.x,
            y: sel.y - window_rect.y,
            w: sel.w,
            h: sel.h,
        })
        .unwrap_or_else(|| util::Rect {
            x: 0,
            y: 0,
            w: window_rect.w,
            h: window_rect.h,
        });

    let image = display
        .get_image(window, sel, xwrap::ALL_PLANES, xlib::ZPixmap)
        .ok_or_else(|| {
            eprintln!("Failed to get image from X");
            1
        })?;

    let mut image =
        image::DynamicImage::ImageRgba8(image.into_image_buffer().ok_or_else(|| {
            eprintln!(
                "Failed to convert captured framebuffer, only 24/32 \
                   bit (A)RGB8 is supported"
            );
            1
        })?);

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

    let output_ext = parsed_opts.output_ext;
    match parsed_opts.path.unwrap_or_else(|| {
        let ts_path = timestamp_path(&output_ext);
        eprintln!("No output specified, defaulting to {}", ts_path.display());
        ParsedPath::Path(ts_path)
    }) {
        ParsedPath::Stdout => {
            let stdout = io::stdout();
            let mut writer = io::BufWriter::new(stdout.lock());
            let err_msg = "Writing to stdout failed";
            image
                .write_to(&mut writer, parsed_opts.output_format)
                .expect(err_msg);
            writer.flush().expect(err_msg);
        }
        ParsedPath::Path(p) => {
            let mut writer = io::BufWriter::new(File::create(&p).map_err(|e| {
                eprintln!("Failed to create {}: {}", p.display(), e);
                1
            })?);
            let err_msg = "Writing to file failed";
            image
                .write_to(&mut writer, parsed_opts.output_format)
                .expect(err_msg);
            writer.flush().expect(err_msg);
        }
    }

    Ok(0)
}

fn main() {
    process::exit(run().map_or_else(|v| v, |v| v));
}
