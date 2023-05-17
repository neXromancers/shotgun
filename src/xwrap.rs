// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::ffi;
use std::os::raw;
use std::ptr;
use std::slice;

use image::Rgba;
use image::RgbaImage;
use x11::xlib;
use x11::xlib_xcb;
use x11rb::connection::Connection;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{self, ConnectionExt as _};
use x11rb::xcb_ffi::XCBConnection;

use crate::util;

pub struct Display {
    handle: *mut xlib::Display,
    conn: XCBConnection,
    screen: usize,
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

            // TODO: change to RustConnection once the port is complete
            let xcb_conn = xlib_xcb::XGetXCBConnection(d);
            let conn = XCBConnection::from_raw_xcb_connection(xcb_conn, false).ok()?;

            let screen = xlib::XDefaultScreen(d) as _;

            if d.is_null() {
                None
            } else {
                Some(Display {
                    handle: d,
                    conn,
                    screen,
                })
            }
        }
    }

    fn screen(&self) -> &xproto::Screen {
        &self.conn.setup().roots[self.screen]
    }

    pub fn root(&self) -> xproto::Window {
        self.screen().root
    }

    pub fn get_window_geometry(&self, window: xproto::Window) -> Option<util::Rect> {
        let geometry_cookie = self.conn.get_geometry(window).ok()?;
        let tree_cookie = self.conn.query_tree(window).ok()?;
        let geometry = geometry_cookie.reply().ok()?;
        let tree = tree_cookie.reply().ok()?;

        if tree.parent != 0 {
            let cookie = self
                .conn
                .translate_coordinates(tree.parent, tree.root, geometry.x, geometry.y)
                .ok()?;
            let coords = cookie.reply().ok()?;

            Some(util::Rect {
                x: coords.dst_x as i32,
                y: coords.dst_y as i32,
                w: geometry.width as i32,
                h: geometry.height as i32,
            })
        } else {
            Some(util::Rect {
                x: geometry.x as i32,
                y: geometry.y as i32,
                w: geometry.width as i32,
                h: geometry.height as i32,
            })
        }
    }

    pub fn get_image(&self, window: xproto::Window, rect: util::Rect) -> Option<Image> {
        unsafe {
            let all_planes = !0;
            let image = xlib::XGetImage(
                self.handle,
                window as _,
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

    pub fn get_screen_rects(&self) -> Option<Vec<util::Rect>> {
        let cookie = self
            .conn
            .randr_get_screen_resources_current(self.root())
            .ok()?;
        let res = cookie.reply().ok()?;

        let rects = res
            .crtcs
            .iter()
            .map(|&crtc| {
                let cookie = self
                    .conn
                    .randr_get_crtc_info(crtc, res.config_timestamp)
                    .expect("Invalid CRTC info request");
                let info = cookie.reply().expect("Failed to get CRTC info");
                util::Rect {
                    x: info.x as i32,
                    y: info.y as i32,
                    w: info.width as i32,
                    h: info.height as i32,
                }
            })
            .collect::<Vec<_>>();
        Some(rects)
    }

    pub fn get_cursor_position(&self) -> Option<util::Point> {
        let cookie = self.conn.query_pointer(self.root()).ok()?;
        let pointer = cookie.reply().ok()?;

        Some(util::Point {
            x: pointer.win_x as i32,
            y: pointer.win_y as i32,
        })
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
