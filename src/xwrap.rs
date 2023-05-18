// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::ffi;
use std::os::raw;
use std::ptr;

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
    w: u32,
    h: u32,
    format: xproto::Format,
    visual: xproto::Visualtype,
    byte_order: xproto::ImageOrder,
    data: Vec<u8>,
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

    fn find_visual(&self, id: xproto::Visualid) -> Option<&xproto::Visualtype> {
        for screen in &self.conn.setup().roots {
            for depth in &screen.allowed_depths {
                for visual in &depth.visuals {
                    if visual.visual_id == id {
                        return Some(visual);
                    }
                }
            }
        }
        None
    }

    pub fn get_image(&self, window: xproto::Window, rect: util::Rect) -> Option<Image> {
        const ALL_PLANES: u32 = !0;
        let cookie = self
            .conn
            .get_image(
                xproto::ImageFormat::Z_PIXMAP,
                window,
                rect.x as i16,
                rect.y as i16,
                rect.w as u16,
                rect.h as u16,
                ALL_PLANES,
            )
            .ok()?;
        let img = cookie.reply().ok()?;

        let format = *self
            .conn
            .setup()
            .pixmap_formats
            .iter()
            .find(|f| f.depth == img.depth)?;
        let visual = *self.find_visual(img.visual)?;
        let byte_order = self.conn.setup().image_byte_order;

        Some(Image {
            w: rect.w as u32,
            h: rect.h as u32,
            format,
            visual,
            byte_order,
            data: img.data,
        })
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
    pub fn to_image_buffer(&self) -> Option<RgbaImage> {
        if (
            self.visual.red_mask,
            self.visual.green_mask,
            self.visual.blue_mask,
        ) == (0xF800, 0x07E0, 0x001F)
        {
            return self.to_image_buffer_rgb565();
        }

        let bytes_per_pixel = match (self.format.depth, self.format.bits_per_pixel) {
            (24, bpp @ 24) | (24 | 32, bpp @ 32) => bpp as u32 / 8,
            _ => return None,
        };

        let pad = match self.format.scanline_pad {
            p @ 32 => p as u32 / 8,
            _ => return None,
        };
        let bytes_per_line = (self.w * bytes_per_pixel + pad - 1) / pad * pad;

        // Compute subpixel offsets into each pixel according the the bitmasks X gives us
        // Only 8 bit, byte-aligned values are supported
        // Truncate masks to the lower 32 bits as that is the maximum pixel size
        macro_rules! channel_offset {
            ($mask:expr) => {{
                use xproto::ImageOrder as O;
                match (self.byte_order, $mask & 0xFFFFFFFF) {
                    (O::LSB_FIRST, 0xFF) | (O::MSB_FIRST, 0xFF000000) => 0,
                    (O::LSB_FIRST, 0xFF00) | (O::MSB_FIRST, 0xFF0000) => 1,
                    (O::LSB_FIRST, 0xFF0000) | (O::MSB_FIRST, 0xFF00) => 2,
                    (O::LSB_FIRST, 0xFF000000) | (O::MSB_FIRST, 0xFF) => 3,
                    _ => return None,
                }
            }};
        }
        let red_offset = channel_offset!(self.visual.red_mask);
        let green_offset = channel_offset!(self.visual.green_mask);
        let blue_offset = channel_offset!(self.visual.blue_mask);
        let alpha_offset = channel_offset!(
            !(self.visual.red_mask | self.visual.green_mask | self.visual.blue_mask)
        );

        // Finally, generate the image object
        Some(RgbaImage::from_fn(self.w, self.h, |x, y| {
            let offset = (y * bytes_per_line + x * bytes_per_pixel) as usize;
            Rgba([
                self.data[offset + red_offset],
                self.data[offset + green_offset],
                self.data[offset + blue_offset],
                // Make the alpha channel fully opaque if none is provided
                if self.format.depth == 24 {
                    0xFF
                } else {
                    self.data[offset + alpha_offset]
                },
            ])
        }))
    }

    fn to_image_buffer_rgb565(&self) -> Option<RgbaImage> {
        if self.format.depth != 16 || self.format.bits_per_pixel != 16 {
            return None;
        }
        let bytes_per_pixel = 2;

        let pad = match self.format.scanline_pad {
            p @ (16 | 32) => p as u32 / 8,
            _ => return None,
        };
        let bytes_per_line = (self.w * bytes_per_pixel + pad - 1) / pad * pad;

        // Finally, generate the image object
        Some(RgbaImage::from_fn(self.w, self.h, |x, y| {
            let offset = (y * bytes_per_line + x * bytes_per_pixel) as usize;
            let pixel_slice = [self.data[offset], self.data[offset + 1]];
            let pixel = if self.byte_order == xproto::ImageOrder::LSB_FIRST {
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
        }))
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
