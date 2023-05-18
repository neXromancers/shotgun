// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::ffi;
use std::mem;
use std::os::raw;
use std::ptr;
use std::slice;

use image::Rgba;
use image::RgbaImage;
use x11::xlib;
use x11::xrandr;

use crate::util;

pub struct Display {
    handle: *mut xlib::Display,
}

pub struct Image {
    handle: *mut xlib::XImage,
}

pub struct ScreenRectIter<'a> {
    dpy: &'a Display,
    res: *mut xrandr::XRRScreenResources,
    crtcs: &'a [xrandr::RRCrtc],
    i: usize,
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
                None
            } else {
                Some(Display { handle: d })
            }
        }
    }

    pub fn root(&self) -> xlib::Window {
        unsafe { xlib::XDefaultRootWindow(self.handle) }
    }

    pub fn get_window_rect(&self, window: xlib::Window) -> util::Rect {
        unsafe {
            let mut attrs = mem::MaybeUninit::uninit();
            xlib::XGetWindowAttributes(self.handle, window, attrs.as_mut_ptr());
            let attrs = attrs.assume_init();

            let mut root = 0;
            let mut parent = 0;
            let mut children: *mut xlib::Window = ptr::null_mut();
            let mut nchildren = 0;
            xlib::XQueryTree(
                self.handle,
                window,
                &mut root,
                &mut parent,
                &mut children,
                &mut nchildren,
            );
            if !children.is_null() {
                xlib::XFree(children as *mut raw::c_void);
            }

            let mut x = attrs.x;
            let mut y = attrs.y;

            if parent != 0 {
                let mut child = 0;
                xlib::XTranslateCoordinates(
                    self.handle,
                    parent,
                    root,
                    attrs.x,
                    attrs.y,
                    &mut x,
                    &mut y,
                    &mut child,
                );
            }

            util::Rect {
                x,
                y,
                w: attrs.width,
                h: attrs.height,
            }
        }
    }

    pub fn get_image(&self, window: xlib::Window, rect: util::Rect) -> Option<Image> {
        unsafe {
            let all_planes = !0;
            let image = xlib::XGetImage(
                self.handle,
                window,
                rect.x,
                rect.y,
                rect.w as std::ffi::c_uint,
                rect.h as std::ffi::c_uint,
                all_planes,
                xlib::ZPixmap,
            );

            Image::from_raw_ximage(image)
        }
    }

    pub fn get_screen_rects(&self) -> Option<ScreenRectIter<'_>> {
        unsafe {
            let xrr_res = xrandr::XRRGetScreenResourcesCurrent(self.handle, self.root());

            if xrr_res.is_null() {
                None
            } else {
                Some(ScreenRectIter {
                    dpy: self,
                    res: xrr_res,
                    crtcs: slice::from_raw_parts((*xrr_res).crtcs, (*xrr_res).ncrtc as usize),
                    i: 0,
                })
            }
        }
    }

    pub fn get_cursor_position(&self) -> Option<util::Point> {
        let mut x = 0;
        let mut y = 0;

        unsafe {
            if xlib::XQueryPointer(
                self.handle,
                self.root(),
                &mut 0,
                &mut 0,
                &mut x,
                &mut y,
                &mut 0,
                &mut 0,
                &mut 0,
            ) == 0
            {
                None
            } else {
                Some(util::Point { x, y })
            }
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
    fn from_raw_ximage(ximage: *mut xlib::XImage) -> Option<Image> {
        if ximage.is_null() {
            None
        } else {
            Some(Image { handle: ximage })
        }
    }

    pub fn to_image_buffer(&self) -> Option<RgbaImage> {
        let img = unsafe { &*self.handle };

        if (img.red_mask, img.green_mask, img.blue_mask) == (0xF800, 0x07E0, 0x001F) {
            return self.to_image_buffer_rgb565();
        }

        let bytes_per_pixel = match (img.depth, img.bits_per_pixel) {
            (24, 24) => 3,
            (24, 32) | (32, 32) => 4,
            _ => return None,
        };

        // Compute subpixel offsets into each pixel according the the bitmasks X gives us
        // Only 8 bit, byte-aligned values are supported
        // Truncate masks to the lower 32 bits as that is the maximum pixel size
        macro_rules! channel_offset {
            ($mask:expr) => {
                match (img.byte_order, $mask & 0xFFFFFFFF) {
                    (0, 0xFF) | (1, 0xFF000000) => 0,
                    (0, 0xFF00) | (1, 0xFF0000) => 1,
                    (0, 0xFF0000) | (1, 0xFF00) => 2,
                    (0, 0xFF000000) | (1, 0xFF) => 3,
                    _ => return None,
                }
            };
        }
        let red_offset = channel_offset!(img.red_mask);
        let green_offset = channel_offset!(img.green_mask);
        let blue_offset = channel_offset!(img.blue_mask);
        let alpha_offset = channel_offset!(!(img.red_mask | img.green_mask | img.blue_mask));

        // Wrap the pixel buffer into a slice
        let size = (img.bytes_per_line * img.height) as usize;
        let data = unsafe { slice::from_raw_parts(img.data as *const u8, size) };

        // Finally, generate the image object
        Some(RgbaImage::from_fn(
            img.width as u32,
            img.height as u32,
            |x, y| {
                let offset = (y * img.bytes_per_line as u32 + x * bytes_per_pixel) as usize;
                Rgba([
                    data[offset + red_offset],
                    data[offset + green_offset],
                    data[offset + blue_offset],
                    // Make the alpha channel fully opaque if none is provided
                    if img.depth == 24 {
                        0xFF
                    } else {
                        data[offset + alpha_offset]
                    },
                ])
            },
        ))
    }

    fn to_image_buffer_rgb565(&self) -> Option<RgbaImage> {
        let img = unsafe { &*self.handle };

        if img.depth != 16 || img.bits_per_pixel != 16 {
            return None;
        }
        let bytes_per_pixel = 2;

        // Wrap the pixel buffer into a slice
        let size = (img.bytes_per_line * img.height) as usize;
        let data = unsafe { slice::from_raw_parts(img.data as *const u8, size) };

        // Finally, generate the image object
        Some(RgbaImage::from_fn(
            img.width as u32,
            img.height as u32,
            |x, y| {
                let offset = (y * img.bytes_per_line as u32 + x * bytes_per_pixel) as usize;
                let pixel_slice = [data[offset], data[offset + 1]];
                let pixel = if img.byte_order == 0 {
                    u16::from_le_bytes(pixel_slice)
                } else {
                    u16::from_be_bytes(pixel_slice)
                };
                let red = (pixel >> 11) & 0x1F;
                let green = (pixel >> 5) & 0x3F;
                let blue = pixel & 0x1F;
                Rgba([
                    (red << 3 | red >> 2) as u8,
                    (green << 2 | green >> 4) as u8,
                    (blue << 3 | blue >> 2) as u8,
                    0xFF,
                ])
            },
        ))
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        unsafe {
            xlib::XDestroyImage(self.handle);
        }
    }
}

impl<'a> Iterator for ScreenRectIter<'a> {
    type Item = util::Rect;

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.crtcs.len() {
            return None;
        }

        unsafe {
            // TODO Handle failure here?
            let crtc = xrandr::XRRGetCrtcInfo(self.dpy.handle, self.res, self.crtcs[self.i]);
            let x = (*crtc).x;
            let y = (*crtc).y;
            let w = (*crtc).width;
            let h = (*crtc).height;
            xrandr::XRRFreeCrtcInfo(crtc);

            self.i += 1;

            //Some((w as i32, h as i32, x as i32, y as i32))
            Some(util::Rect {
                x,
                y,
                w: w as i32,
                h: h as i32,
            })
        }
    }
}

impl<'a> Drop for ScreenRectIter<'a> {
    fn drop(&mut self) {
        unsafe {
            xrandr::XRRFreeScreenResources(self.res);
        }
    }
}

pub fn parse_geometry(g: ffi::CString) -> util::Rect {
    unsafe {
        let mut x = 0;
        let mut y = 0;
        let mut w = 0;
        let mut h = 0;
        xlib::XParseGeometry(
            g.as_ptr() as *const raw::c_char,
            &mut x,
            &mut y,
            &mut w,
            &mut h,
        );

        util::Rect {
            x,
            y,
            w: w as i32,
            h: h as i32,
        }
    }
}
