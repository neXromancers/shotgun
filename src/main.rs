use std::env;
use std::ffi::CString;
use std::fs::File;
use std::io;
use std::path::Path;
use std::process;

extern crate getopts;
extern crate image;
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let progname = args[0].clone();

    let mut opts = getopts::Options::new();
    opts.optopt("i", "id", "Window to capture", "ID");
    opts.optopt("g", "geometry", "Area to capture", "WxH+X+Y");
    opts.optflag("h", "help", "Print help and exit");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("{}", f.to_string());
            usage(&progname, opts);
            process::exit(1);
        }
    };

    // One loose argument allowed (file name)
    if matches.opt_present("h") || matches.free.len() > 1 {
        usage(&progname, opts);
        return;
    }

    let display = match Display::open(None) {
        Some(d) => d,
        None => {
            eprintln!("Failed to open display");
            process::exit(1);
        }
    };
    let root = display.get_default_root();

    let window = matches.opt_str("i").map_or(root, |s| match s.parse::<xlib::Window>() {
        Ok(r) => r,
        Err(_) => {
            eprintln!("Window ID is not a valid integer");
            process::exit(1);
        },
    });

    let (w, h, x, y) = matches.opt_str("g").map_or_else(|| {
        let attrs = display.get_window_attributes(window);
        (attrs.width as libc::c_uint, attrs.height as libc::c_uint, 0, 0)
    },
    |s| {
        xwrap::parse_geometry(CString::new(s).expect("Failed to convert CString"))
    });

    if w <= 0 || h <= 0 {
        eprintln!("Capture dimensions must be greater than 0");
        process::exit(1);
    }

    let image = match display.get_image(window, x, y, w, h, xwrap::ALL_PLANES, xlib::ZPixmap) {
        Some(i) => i,
        None => {
            eprintln!("Failed to get image from X");
            process::exit(1);
        },
    };

    let image = match image.into_image_buffer() {
        Some(i) => image::ImageRgba8(i),
        None => {
            eprintln!("Failed to convert captured framebuffer, only 24/32 \
                      bit (A)RGB8 is supported");
            process::exit(1);
        }
    };

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
                process::exit(1)
            },
        }
    }
}
