use std::{borrow::Cow, io::Write};

#[cfg(not(target_arch = "wasm32"))]
use std::io::Error as IoStdError;

use image::{ExtendedColorType, ImageEncoder, ImageFormat, RgbaImage, codecs::jpeg::JpegEncoder};
use png::{ColorType, Compression, Filter};
use serde::Deserialize;

use image_webp::WebPEncoder;

#[cfg(feature = "rayon")]
use rayon::prelude::*;

use crate::{Error::IoError, Result};

/// Output format for rendered images.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImageOutputFormat {
  /// WebP image format, provides good compression and supports animation.
  /// It is useful for images in web contents.
  WebP,

  /// PNG image format, lossless and widely supported, and its the fastest format to encode.
  Png,

  /// JPEG image format, lossy and does not support transparency.
  Jpeg,
}

impl ImageOutputFormat {
  /// Returns the MIME type for the image output format.
  pub fn content_type(&self) -> &'static str {
    match self {
      ImageOutputFormat::WebP => "image/webp",
      ImageOutputFormat::Png => "image/png",
      ImageOutputFormat::Jpeg => "image/jpeg",
    }
  }
}

impl From<ImageOutputFormat> for ImageFormat {
  fn from(format: ImageOutputFormat) -> Self {
    match format {
      ImageOutputFormat::WebP => Self::WebP,
      ImageOutputFormat::Png => Self::Png,
      ImageOutputFormat::Jpeg => Self::Jpeg,
    }
  }
}

/// Represents a single frame of an animated image.
#[derive(Debug, Clone)]
pub struct AnimationFrame {
  /// The image data for the frame.
  pub image: RgbaImage,
  /// The duration of the frame in milliseconds.
  /// Maximum value is 0xffffff (24-bit), overflow will be clamped.
  pub duration_ms: u32,
}

impl AnimationFrame {
  /// Creates a new animation frame.
  pub fn new(image: RgbaImage, duration_ms: u32) -> Self {
    Self { image, duration_ms }
  }
}

const U24_MAX: u32 = 0xffffff;

fn strip_alpha_channel(image: Cow<'_, RgbaImage>) -> Vec<u8> {
  match image {
    Cow::Owned(image) => {
      let mut rgba = image.into_raw();
      let pixels = rgba.len() / 4;

      for pixel_index in 0..pixels {
        let src_offset = pixel_index * 4;
        let dst_offset = pixel_index * 3;
        rgba[dst_offset] = rgba[src_offset];
        rgba[dst_offset + 1] = rgba[src_offset + 1];
        rgba[dst_offset + 2] = rgba[src_offset + 2];
      }

      rgba.truncate(pixels * 3);
      rgba
    }
    Cow::Borrowed(image) => {
      let pixels = bytemuck::cast_slice::<u8, [u8; 4]>(image.as_raw());
      let mut rgb = Vec::with_capacity(pixels.len() * 3);

      for [r, g, b, _] in pixels {
        rgb.extend_from_slice(&[*r, *g, *b]);
      }

      rgb
    }
  }
}

fn has_any_alpha_pixel(image: &RgbaImage) -> bool {
  bytemuck::cast_slice::<u8, [u8; 4]>(image.as_raw())
    .iter()
    .any(|[_, _, _, a]| *a != u8::MAX)
}

#[cfg(not(target_arch = "wasm32"))]
fn write_webp(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
  quality: Option<u8>,
) -> Result<()> {
  use webp::{Encoder as WebpEncoder, WebPConfig};

  let requested_quality = quality.unwrap_or(100).clamp(0, 100);
  let is_lossless = requested_quality == 100;
  let has_alpha = has_any_alpha_pixel(&image);
  let width = image.width();
  let height = image.height();

  let mut config =
    WebPConfig::new().map_err(|_| IoError(IoStdError::other("Failed to construct WebP config")))?;
  config.lossless = if is_lossless { 1 } else { 0 };
  config.alpha_compression = if is_lossless { 0 } else { 1 };
  config.method = 0;
  config.quality = if is_lossless {
    75.0
  } else {
    requested_quality as f32
  };

  let encoded = if has_alpha {
    WebpEncoder::from_rgba(image.as_raw(), width, height)
      .encode_advanced(&config)
      .map_err(|error| IoError(IoStdError::other(format!("WebP encode error: {error:?}"))))?
  } else {
    let rgb = strip_alpha_channel(image);
    WebpEncoder::from_rgb(&rgb, width, height)
      .encode_advanced(&config)
      .map_err(|error| IoError(IoStdError::other(format!("WebP encode error: {error:?}"))))?
  };

  destination.write_all(&encoded)?;
  Ok(())
}

#[cfg(target_arch = "wasm32")]
fn write_webp(
  image: Cow<'_, RgbaImage>,
  destination: &mut impl Write,
  _quality: Option<u8>,
) -> Result<()> {
  let encoder = WebPEncoder::new(destination);
  let width = image.width();
  let height = image.height();
  let has_alpha = has_any_alpha_pixel(&image);

  let image_data = if has_alpha {
    Cow::Borrowed(image.as_raw())
  } else {
    Cow::Owned(strip_alpha_channel(image))
  };

  encoder.encode(
    &image_data,
    width,
    height,
    if has_alpha {
      image_webp::ColorType::Rgba8
    } else {
      image_webp::ColorType::Rgb8
    },
  )?;

  Ok(())
}

/// Writes a single rendered image to `destination` using `format`.
pub fn write_image<'a, T: Write>(
  image: Cow<'a, RgbaImage>,
  destination: &mut T,
  format: ImageOutputFormat,
  quality: Option<u8>,
) -> Result<()> {
  match format {
    ImageOutputFormat::Jpeg => {
      let width = image.width();
      let height = image.height();
      let rgb = strip_alpha_channel(image);

      let encoder = JpegEncoder::new_with_quality(destination, quality.unwrap_or(75));
      encoder.write_image(&rgb, width, height, ExtendedColorType::Rgb8)?;
    }
    ImageOutputFormat::Png => {
      let mut encoder = png::Encoder::new(destination, image.width(), image.height());

      let has_alpha = has_any_alpha_pixel(&image);

      let image_data = if has_alpha {
        Cow::Borrowed(image.as_raw())
      } else {
        Cow::Owned(strip_alpha_channel(image))
      };

      encoder.set_color(if has_alpha {
        ColorType::Rgba
      } else {
        ColorType::Rgb
      });

      // Use quality settings to determine compression level.
      // Higher quality settings map to better compression ratio (slower).
      // If quality is not specified or < 90, we favor speed.
      let quality = quality.unwrap_or(75);
      if quality >= 90 {
        encoder.set_compression(Compression::Balanced);
      } else {
        encoder.set_compression(Compression::Fast);
      }

      // Fast subtraction filter handles smooth gradients well with minimal overhead.
      encoder.set_filter(Filter::Sub);

      let mut writer = encoder.write_header()?;
      writer.write_image_data(&image_data)?;
      writer.finish()?;
    }
    ImageOutputFormat::WebP => {
      write_webp(image, destination, quality)?;
    }
  }

  Ok(())
}

/// Scans the RIFF container and returns (offset, length) of the VP8/VP8L payload.
/// Returns None if the tag is not found or if the buffer is truncated.
fn vp8_payload_coords(buf: &[u8]) -> Option<(usize, usize)> {
  // Skip RIFF header (12 bytes)
  if buf.len() < 12 {
    return None;
  }

  let mut i = 12;
  let buf_len = buf.len();

  // Iterate over chunks
  while i + 8 <= buf_len {
    let tag = &buf[i..i + 4];

    let len = u32::from_le_bytes(buf[i + 4..i + 8].try_into().ok()?) as usize;

    // Check for VP8 (Lossy) or VP8L (Lossless)
    if tag == b"VP8 " || tag == b"VP8L" {
      let start = i + 8;
      let end = start.checked_add(len)?; // Protect against usize overflow

      // Ensure the actual data exists in the buffer.
      if end > buf_len {
        return None;
      }

      return Some((start, len));
    }

    // Calculate next chunk offset (Size + Padding)
    let padding = len & 1;

    let chunk_size = len.checked_add(padding)?;
    i = (i + 8).checked_add(chunk_size)?;
  }

  None
}

// NAME + size (4 bytes)
const BASE_HEADER_SIZE: u32 = 8;

// x (3 bytes) + y (3 bytes) + w (3 bytes) + h (3 bytes) + duration (3 bytes) + flags (1 byte)
const ANMF_HEADER_SIZE: u32 = 16;

// flags (1 byte) + cw (3 bytes) + ch (3 bytes)
const VP8X_HEADER_SIZE: u32 = 10;

// background color (4 bytes) + loop count (2 bytes)
const ANIM_HEADER_SIZE: u32 = 6;

fn estimate_vp8_payload_size(buf: &[u8]) -> Result<u32> {
  let (_, len) = vp8_payload_coords(buf)
    .ok_or_else(|| IoError(std::io::Error::other("VP8/VP8L chunk not found")))?;

  let padding = len & 1;

  // ANMF chunk + VP8L chunk
  Ok(BASE_HEADER_SIZE + ANMF_HEADER_SIZE + BASE_HEADER_SIZE + len as u32 + padding as u32)
}

fn estimate_riff_size<'a, I: Iterator<Item = &'a [u8]>>(frames: I) -> Result<u32> {
  // "WEBP" +  VPX8 chunk + ANIM chunk + [ANMF chunks]
  let mut size = 4 + BASE_HEADER_SIZE + VP8X_HEADER_SIZE + BASE_HEADER_SIZE + ANIM_HEADER_SIZE;

  for frame in frames {
    size += estimate_vp8_payload_size(frame)?;
  }

  Ok(size)
}

/// Encode a sequence of RGBA frames into an animated WebP and write to `destination`.
pub fn encode_animated_webp<W: Write>(
  frames: &[AnimationFrame],
  destination: &mut W,
  blend: bool,
  dispose: bool,
  loop_count: Option<u16>,
) -> Result<()> {
  assert_ne!(frames.len(), 0);

  // encode frames losslessly and collect VP8L/VP8 payloads
  #[cfg(feature = "rayon")]
  let frames_payloads: Vec<(&AnimationFrame, Vec<u8>)> = frames
    .par_iter()
    .map(|frame| {
      let mut buf = Vec::new();
      WebPEncoder::new(&mut buf).encode(
        &frame.image,
        frame.image.width(),
        frame.image.height(),
        image_webp::ColorType::Rgba8,
      )?;

      Ok((frame, buf))
    })
    .collect::<Result<Vec<(&AnimationFrame, Vec<u8>)>>>()?;

  #[cfg(not(feature = "rayon"))]
  let frames_payloads: Vec<(&AnimationFrame, Vec<u8>)> = frames
    .iter()
    .map(|frame| {
      let mut buf = Vec::new();
      WebPEncoder::new(&mut buf)
        .encode(
          &frame.image,
          frame.image.width(),
          frame.image.height(),
          image_webp::ColorType::Rgba8,
        )
        .map_err(|_| IoError(std::io::Error::other("WebP encode error")))?;

      Ok((frame, buf))
    })
    .collect::<Result<Vec<(&AnimationFrame, Vec<u8>)>>>()?;

  let riff_size = estimate_riff_size(frames_payloads.iter().map(|(_, buf)| buf.as_slice()))?;

  // RIFF header
  destination.write_all(b"RIFF")?;
  destination.write_all(&(riff_size as u32).to_le_bytes())?;
  destination.write_all(b"WEBP")?;

  // VP8X chunk
  let vp8x_flags: u8 = (1 << 1) | (1 << 4); // animation + alpha
  let cw = (frames[0].image.width() - 1).to_le_bytes();
  let ch = (frames[0].image.height() - 1).to_le_bytes();

  destination.write_all(b"VP8X")?;
  destination.write_all(&VP8X_HEADER_SIZE.to_le_bytes())?;
  destination.write_all(&[vp8x_flags])?;
  destination.write_all(&[0u8; 3])?;
  destination.write_all(&cw[..3])?;
  destination.write_all(&ch[..3])?;

  // ANIM chunk
  destination.write_all(b"ANIM")?;
  destination.write_all(&ANIM_HEADER_SIZE.to_le_bytes())?;
  destination.write_all(&[0u8; 4])?; // bgcolor (4 bytes)
  destination.write_all(&loop_count.unwrap_or(0).to_le_bytes())?;

  let frame_flags = ((blend as u8) << 1) | (dispose as u8);

  // ANMF frames
  for (frame, vp8_data) in frames_payloads.into_iter() {
    let w_bytes = (frame.image.width() - 1).to_le_bytes();
    let h_bytes = (frame.image.height() - 1).to_le_bytes();

    let (start, len) = vp8_payload_coords(&vp8_data)
      .ok_or_else(|| IoError(std::io::Error::other("VP8/VP8L chunk not found")))?;

    let vp8_payload = &vp8_data[start..start + len];

    let padding = vp8_payload.len() & 1;

    let anmf_size = ANMF_HEADER_SIZE + BASE_HEADER_SIZE + vp8_payload.len() as u32 + padding as u32; // x, y, w, h, duration, flags, payload

    destination.write_all(b"ANMF")?;
    destination.write_all(&anmf_size.to_le_bytes())?;

    // frame header (16 bytes)
    destination.write_all(&[0u8; 6])?; // x, y (3 bytes each)
    destination.write_all(&w_bytes[..3])?; // w (3 bytes)
    destination.write_all(&h_bytes[..3])?; // h (3 bytes)
    destination.write_all(&frame.duration_ms.clamp(0, U24_MAX).to_le_bytes()[..3])?; // duration (3 bytes)
    destination.write_all(&[frame_flags])?; // flags (1 byte)

    // VP8L chunk: VP8L payload
    destination.write_all(b"VP8L")?;
    destination.write_all(&(vp8_payload.len() as u32).to_le_bytes())?;
    destination.write_all(vp8_payload)?;

    // padding
    if padding == 1 {
      destination.write_all(&[0u8])?;
    }
  }

  destination.flush()?;

  Ok(())
}

/// Encode a sequence of RGBA frames into an animated PNG and write to `destination`.
pub fn encode_animated_png<W: Write>(
  frames: &[AnimationFrame],
  destination: &mut W,
  loop_count: Option<u16>,
) -> Result<()> {
  assert_ne!(frames.len(), 0);

  let mut encoder = png::Encoder::new(
    destination,
    frames[0].image.width(),
    frames[0].image.height(),
  );

  encoder.set_color(ColorType::Rgba);
  encoder.set_compression(png::Compression::Fastest);
  encoder.set_animated(frames.len() as u32, loop_count.unwrap_or(0) as u32)?;

  // Since APNG doesn't support variable frame duration, we use the minimum duration of all frames.
  let min_duration_ms = frames
    .iter()
    .map(|frame| frame.duration_ms)
    .min()
    .unwrap_or(0);

  encoder.set_frame_delay(min_duration_ms.clamp(0, u16::MAX as u32) as u16, 1000)?;

  let mut writer = encoder.write_header()?;

  for frame in frames {
    writer.write_image_data(frame.image.as_raw())?;
  }

  writer.finish()?;

  Ok(())
}
