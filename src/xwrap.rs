use std::ffi;
use std::mem;
use std::ptr;
use std::slice;

use libc;
use x11::xlib;

pub const ALL_PLANES: libc::c_ulong = !0;

macro_rules! get {
    ($s:ident, $a:ident) => ((*$s.handle).$a);
}

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

            Some(Image {
                handle: image,
            })
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
    pub fn get_data(&self) -> &[u8] {
        unsafe {
            let length = (get!(self, width) * get!(self, height) * 4) as usize;
            slice::from_raw_parts(get!(self, data) as *const u8, length)
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
