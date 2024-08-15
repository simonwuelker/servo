/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::borrow::Cow;
use std::fmt;

use euclid::default::{Point2D, Rect, Size2D};
use ipc_channel::ipc::IpcSharedMemory;
use log::debug;
use malloc_size_of_derive::MallocSizeOf;
use serde::{Deserialize, Serialize};
use webrender_api::ImageKey;

#[derive(Clone, Copy, Debug, Deserialize, Eq, MallocSizeOf, PartialEq, Serialize)]
pub enum PixelFormat {
    /// Luminance channel only
    K8,
    /// Luminance + alpha
    KA8,
    /// RGB, 8 bits per channel
    RGB8,
    /// RGB + alpha, 8 bits per channel
    RGBA8,
    /// BGR + alpha, 8 bits per channel
    BGRA8,
}

pub fn rgba8_get_rect(pixels: &[u8], size: Size2D<u64>, rect: Rect<u64>) -> Cow<[u8]> {
    assert!(!rect.is_empty());
    assert!(Rect::from_size(size).contains_rect(&rect));
    assert_eq!(pixels.len() % 4, 0);
    assert_eq!(size.area() as usize, pixels.len() / 4);
    let area = rect.size.area() as usize;
    let first_column_start = rect.origin.x as usize * 4;
    let row_length = size.width as usize * 4;
    let first_row_start = rect.origin.y as usize * row_length;
    if rect.origin.x == 0 && rect.size.width == size.width || rect.size.height == 1 {
        let start = first_column_start + first_row_start;
        return Cow::Borrowed(&pixels[start..start + area * 4]);
    }
    let mut data = Vec::with_capacity(area * 4);
    for row in pixels[first_row_start..]
        .chunks(row_length)
        .take(rect.size.height as usize)
    {
        data.extend_from_slice(&row[first_column_start..][..rect.size.width as usize * 4]);
    }
    data.into()
}

// TODO(pcwalton): Speed up with SIMD, or better yet, find some way to not do this.
pub fn rgba8_byte_swap_colors_inplace(pixels: &mut [u8]) {
    assert!(pixels.len() % 4 == 0);
    for rgba in pixels.chunks_mut(4) {
        rgba.swap(0, 2);
    }
}

pub fn rgba8_byte_swap_and_premultiply_inplace(pixels: &mut [u8]) {
    assert!(pixels.len() % 4 == 0);
    for rgba in pixels.chunks_mut(4) {
        let b = rgba[0];
        rgba[0] = multiply_u8_color(rgba[2], rgba[3]);
        rgba[1] = multiply_u8_color(rgba[1], rgba[3]);
        rgba[2] = multiply_u8_color(b, rgba[3]);
    }
}

/// Returns true if the pixels were found to be completely opaque.
pub fn rgba8_premultiply_inplace(pixels: &mut [u8]) -> bool {
    assert!(pixels.len() % 4 == 0);
    let mut is_opaque = true;
    for rgba in pixels.chunks_mut(4) {
        rgba[0] = multiply_u8_color(rgba[0], rgba[3]);
        rgba[1] = multiply_u8_color(rgba[1], rgba[3]);
        rgba[2] = multiply_u8_color(rgba[2], rgba[3]);
        is_opaque = is_opaque && rgba[3] == 255;
    }
    is_opaque
}

pub fn multiply_u8_color(a: u8, b: u8) -> u8 {
    (a as u32 * b as u32 / 255) as u8
}

pub fn clip(
    mut origin: Point2D<i32>,
    mut size: Size2D<u64>,
    surface: Size2D<u64>,
) -> Option<Rect<u64>> {
    if origin.x < 0 {
        size.width = size.width.saturating_sub(-origin.x as u64);
        origin.x = 0;
    }
    if origin.y < 0 {
        size.height = size.height.saturating_sub(-origin.y as u64);
        origin.y = 0;
    }
    let origin = Point2D::new(origin.x as u64, origin.y as u64);
    Rect::new(origin, size)
        .intersection(&Rect::from_size(surface))
        .filter(|rect| !rect.is_empty())
}

/// Whether this response passed any CORS checks, and is thus safe to read from
/// in cross-origin environments.
#[derive(Clone, Copy, Debug, Deserialize, MallocSizeOf, PartialEq, Serialize)]
pub enum CorsStatus {
    /// The response is either same-origin or cross-origin but passed CORS checks.
    Safe,
    /// The response is cross-origin and did not pass CORS checks. It is unsafe
    /// to expose pixel data to the requesting environment.
    Unsafe,
}

#[derive(Clone, Deserialize, MallocSizeOf, Serialize)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    #[ignore_malloc_size_of = "Defined in ipc-channel"]
    pub bytes: IpcSharedMemory,
    #[ignore_malloc_size_of = "Defined in webrender_api"]
    pub id: Option<ImageKey>,
    pub cors_status: CorsStatus,
}

impl fmt::Debug for Image {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Image {{ width: {}, height: {}, format: {:?}, ..., id: {:?} }}",
            self.width, self.height, self.format, self.id
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, MallocSizeOf, PartialEq, Serialize)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
}

// FIXME: Images must not be copied every frame. Instead we should atomically
// reference count them.

#[must_use]
pub fn load_from_memory(buffer: &[u8], cors_status: CorsStatus) -> Option<Image> {
    if buffer.is_empty() {
        return None;
    }

    match image::load_from_memory(buffer) {
        Ok(image) => {
            let mut rgba = image.into_rgba8();
            rgba8_byte_swap_colors_inplace(&mut rgba);
            Some(Image {
                width: rgba.width(),
                height: rgba.height(),
                format: PixelFormat::BGRA8,
                bytes: IpcSharedMemory::from_bytes(&rgba),
                id: None,
                cors_status,
            })
        },
        Err(e) => {
            debug!("Image decoding error: {:?}", e);
            None
        },
    }
}
