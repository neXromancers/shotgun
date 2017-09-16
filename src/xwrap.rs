use std::ffi;
use std::mem;
use std::ptr;
use std::slice;

use image::{RgbaImage, Pixel, Rgba};
use libc;
use x11::xlib;

pub const ALL_PLANES: libc::c_ulong = !0;

pub struct Display {
    handle: *mut xlib::Display,
}

pub struct Image {
    handle: *mut xlib::XImage,
}

impl Display {
    pub fn open(name: Option<ffi::CString>) -> Option<Display> {
        unsafe {
            let name = match name {
                None => ptr::null(),
                Some(cstr) => cstr.as_ptr(),
            };
            let d = xlib::XOpenDisplay(name);

            if d.is_null() {
                return None;
            }

            Some(Display {
                handle: d,
            })
        }
    }

    pub fn get_default_root(&self) -> xlib::Window {
        unsafe {
            xlib::XDefaultRootWindow(self.handle)
        }
    }

    pub fn get_window_attributes(&self, window: xlib::Window) -> xlib::XWindowAttributes {
        unsafe {
            let mut attrs = mem::uninitialized();
            xlib::XGetWindowAttributes(self.handle, window, &mut attrs);
            attrs
        }
    }

    pub fn get_image(&self, window: xlib::Window,
                     x: libc::c_int, y: libc::c_int,
                     w: libc::c_uint, h: libc::c_uint,
                     plane_mask: libc::c_ulong,
                     format: libc::c_int) -> Option<Image> {
        unsafe {
            let image = xlib::XGetImage(self.handle, window, x, y, w, h, plane_mask, format);

            if image.is_null() {
                return None;
            }

            Some(Image::from_raw_ximage(image))
        }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        unsafe {
            xlib::XCloseDisplay(self.handle);
        }
    }
}

impl Image {
    pub fn from_raw_ximage(ximage: *mut xlib::XImage) -> Image {
        Image {
            handle: ximage,
        }
    }

    pub fn into_image_buffer(&self) -> Option<RgbaImage> {
        unsafe {
            // Extract values from the XImage into our own scope
            macro_rules! get {
                ($($a:ident),+) => ($(let $a = (*self.handle).$a;)+);
            }
            get!(width, height,
                 byte_order, depth, bytes_per_line, bits_per_pixel,
                 red_mask, green_mask, blue_mask);

            // Pixel size
            let stride = match (depth, bits_per_pixel) {
                (24, 24) => 3,
                (24, 32) | (32, 32) => 4,
                _ => return None,
            };

            // Compute subpixel offsets into each pixel according the the bitmasks X gives us
            // Only 8 bit, byte-aligned values are supported
            // Truncate masks to the lower 32 bits as that is the maximum pixel size
            macro_rules! channel_offset {
                ($mask:expr) => (match (byte_order, $mask & 0xFFFFFFFF) {
                    (0, 0xFF) | (1, 0xFF000000) => 0,
                    (0, 0xFF00) | (1, 0xFF0000) => 1,
                    (0, 0xFF0000) | (1, 0xFF00) => 2,
                    (0, 0xFF000000) | (1, 0xFF) => 3,
                    _ => return None,
                })
            }
            let red_offset = channel_offset!(red_mask);
            let green_offset = channel_offset!(green_mask);
            let blue_offset = channel_offset!(blue_mask);
            let alpha_offset = channel_offset!(!(red_mask | green_mask | blue_mask));

            // Wrap the pixel buffer into a slice
            let size = (bytes_per_line * height) as usize;
            let data = slice::from_raw_parts((*self.handle).data as *const u8, size);

            // Finally, generate the image object
            Some(RgbaImage::from_fn(width as u32, height as u32, |x, y| {
                macro_rules! subpixel {
                    ($channel_offset:ident) => (data[(y * bytes_per_line as u32
                                                + x * stride as u32
                                                + $channel_offset) as usize]);
                }
                Rgba::from_channels(subpixel!(red_offset),
                                    subpixel!(green_offset),
                                    subpixel!(blue_offset),
                                    // Make the alpha channel fully opaque if none is provided
                                    if depth == 24 { 0xFF } else { subpixel!(alpha_offset) })
            }))
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            xlib::XDestroyImage(self.handle);
        }
    }
}

pub fn parse_geometry(g: ffi::CString) -> (libc::c_uint, libc::c_uint, libc::c_int, libc::c_int) {
    unsafe {
        let mut w = 0;
        let mut h = 0;
        let mut x = 0;
        let mut y = 0;
        xlib::XParseGeometry(g.as_ptr() as *const i8, &mut x, &mut y, &mut w, &mut h);
        (w, h, x, y)
    }
}
