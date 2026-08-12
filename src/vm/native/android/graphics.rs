//! android.graphics host shims: Bitmap, Canvas, Paint, Rect and BitmapFactory.
//! PNG is really decoded/encoded via the `png` crate; JPEG/WEBP are
//! dimension-only (pixels stay empty) with a warning, matching the host
//! scope of the VM.

use super::*;
use crate::vm::native::output_stream_append;

fn bitmap_parts(vm: &mut Vm, v: JValue) -> Result<(i32, i32, Vec<u32>), NatErr> {
    match payload(vm, v) {
        Some(Native::Bitmap {
            width,
            height,
            pixels,
        }) => Ok((*width, *height, pixels.clone())),
        _ => Err(npe(vm)),
    }
}

fn bitmap_mut(vm: &mut Vm, v: JValue) -> Result<&mut Native, NatErr> {
    if !matches!(payload(vm, v), Some(Native::Bitmap { .. })) {
        return Err(npe(vm));
    }
    Ok(payload_mut(vm, v).expect("payload checked"))
}

fn bitmap_alloc(vm: &mut Vm, width: i32, height: i32, pixels: Vec<u32>) -> R {
    alloc(
        vm,
        "Landroid/graphics/Bitmap;",
        Native::Bitmap {
            width,
            height,
            pixels,
        },
    )
}

fn bitmap_rect_ok(
    vm: &mut Vm,
    width: i32,
    height: i32,
    region: (i32, i32, i32, i32, i32, i32),
) -> Result<(i32, i32), NatErr> {
    let (offset, stride, x, y, w, h) = region;
    if stride <= 0 || w < 0 || h < 0 || x < 0 || y < 0 || x + w > width || y + h > height {
        return Err(iae(vm, "pixel region out of bounds"));
    }
    Ok((offset, stride))
}

pub(crate) fn bitmap_get_width(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(bitmap_parts(vm, args[0])?.0))
}

pub(crate) fn bitmap_get_height(vm: &mut Vm, args: &[JValue]) -> R {
    Ok(JValue::Int(bitmap_parts(vm, args[0])?.1))
}

pub(crate) fn bitmap_get_pixel(vm: &mut Vm, args: &[JValue]) -> R {
    let (width, height, pixels) = bitmap_parts(vm, args[0])?;
    let x = int_of(vm, args[1]);
    let y = int_of(vm, args[2]);
    if x < 0 || y < 0 || x >= width || y >= height {
        return Err(aioobe(vm, x.max(y), width.max(height).max(1)));
    }
    Ok(JValue::Int(pixels[(y * width + x) as usize] as i32))
}

pub(crate) fn bitmap_get_pixels(vm: &mut Vm, args: &[JValue]) -> R {
    let (width, height, pixels) = bitmap_parts(vm, args[0])?;
    let offset = int_of(vm, args[2]);
    let stride = int_of(vm, args[3]);
    let x = int_of(vm, args[4]);
    let y = int_of(vm, args[5]);
    let w = int_of(vm, args[6]);
    let h = int_of(vm, args[7]);
    let (offset, stride) = bitmap_rect_ok(vm, width, height, (offset, stride, x, y, w, h))?;
    let Some(Native::Array(ArrayData::Int(vals))) = payload_mut(vm, args[1]) else {
        return Err(npe(vm));
    };
    for row in 0..h {
        for col in 0..w {
            let src = ((y + row) * width + x + col) as usize;
            let dst = offset as usize + (row as usize * stride as usize + col as usize);
            let Some(slot) = vals.get_mut(dst) else {
                return Err(iae(vm, "getPixels destination too small"));
            };
            *slot = pixels[src] as i32;
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn bitmap_set_pixels(vm: &mut Vm, args: &[JValue]) -> R {
    let (width, height, _) = bitmap_parts(vm, args[0])?;
    let offset = int_of(vm, args[2]);
    let stride = int_of(vm, args[3]);
    let x = int_of(vm, args[4]);
    let y = int_of(vm, args[5]);
    let w = int_of(vm, args[6]);
    let h = int_of(vm, args[7]);
    let (offset, stride) = bitmap_rect_ok(vm, width, height, (offset, stride, x, y, w, h))?;
    let Some(Native::Array(ArrayData::Int(vals))) = payload(vm, args[1]) else {
        return Err(npe(vm));
    };
    let vals = vals.clone();
    let src_max = offset + (h - 1) * stride + (w - 1);
    if src_max >= vals.len() as i32 {
        return Err(iae(vm, "setPixels source too small"));
    }
    let Native::Bitmap { pixels, .. } = bitmap_mut(vm, args[0])? else {
        unreachable!("payload checked")
    };
    for row in 0..h {
        for col in 0..w {
            let src = offset as usize + (row as usize * stride as usize + col as usize);
            pixels[((y + row) * width + x + col) as usize] = vals[src] as u32;
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn bitmap_recycle(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn bitmap_erase_color(vm: &mut Vm, args: &[JValue]) -> R {
    let color = int_of(vm, args[1]) as u32;
    let Native::Bitmap { pixels, .. } = bitmap_mut(vm, args[0])? else {
        unreachable!("payload checked")
    };
    pixels.fill(color);
    Ok(JValue::Null)
}

pub(crate) fn bitmap_copy(vm: &mut Vm, args: &[JValue]) -> R {
    let (width, height, pixels) = bitmap_parts(vm, args[0])?;
    bitmap_alloc(vm, width, height, pixels)
}

pub(crate) fn bitmap_create_empty(vm: &mut Vm, args: &[JValue]) -> R {
    let width = int_of(vm, args[0]);
    let height = int_of(vm, args[1]);
    if width <= 0 || height <= 0 {
        return Err(iae(vm, "width and height must be > 0"));
    }
    bitmap_alloc(vm, width, height, vec![0; (width * height) as usize])
}

pub(crate) fn bitmap_crop(vm: &mut Vm, args: &[JValue]) -> R {
    let (width, height, pixels) = bitmap_parts(vm, args[0])?;
    let x = int_of(vm, args[1]);
    let y = int_of(vm, args[2]);
    let w = int_of(vm, args[3]);
    let h = int_of(vm, args[4]);
    if x < 0 || y < 0 || w < 0 || h < 0 || x + w > width || y + h > height {
        return Err(iae(vm, "crop region out of bounds"));
    }
    let mut cropped = Vec::with_capacity((w * h) as usize);
    for row in 0..h {
        let start = ((y + row) * width + x) as usize;
        cropped.extend_from_slice(&pixels[start..start + w as usize]);
    }
    bitmap_alloc(vm, w, h, cropped)
}

pub(crate) fn bitmap_compress(vm: &mut Vm, args: &[JValue]) -> R {
    let (width, height, pixels) = bitmap_parts(vm, args[0])?;
    let format = match args[1] {
        JValue::Int(i) => i,
        JValue::Obj(_) => match payload(vm, args[1]) {
            Some(Native::Enum { ordinal, .. }) => *ordinal,
            _ => -1,
        },
        _ => -1,
    };
    if width <= 0 || height <= 0 {
        return Ok(JValue::Int(0));
    }
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for &px in &pixels {
        rgba.push(((px >> 16) & 0xFF) as u8);
        rgba.push(((px >> 8) & 0xFF) as u8);
        rgba.push((px & 0xFF) as u8);
        rgba.push(((px >> 24) & 0xFF) as u8);
    }
    let encoded = match format {
        1 => match png_encode_rgba(width as u32, height as u32, &rgba) {
            Ok(bytes) => bytes,
            Err(_) => return Ok(JValue::Int(0)),
        },
        _ => {
            log::warn!(
                "Bitmap.compress format {format} not supported by the host; writing raw RGBA stub"
            );
            let mut stub = Vec::with_capacity(12 + rgba.len());
            stub.extend_from_slice(b"DEXVMRAW\0\x01");
            stub.extend_from_slice(&width.to_le_bytes());
            stub.extend_from_slice(&height.to_le_bytes());
            stub.extend_from_slice(&rgba);
            stub
        }
    };
    output_stream_append(vm, args[2], &encoded)?;
    Ok(JValue::Int(1))
}

fn png_encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgba).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

/// Decode PNG or JPEG bytes to RGBA using zune decoders. Returns
/// `(width, height, rgba)` or an error string.
fn decode_image_rgba(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return png_decoded_zune(bytes);
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return jpeg_decoded_zune(bytes);
    }
    Err("unsupported image format".into())
}

fn png_decoded_zune(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    use zune_png::zune_core::options::DecoderOptions;
    let options = DecoderOptions::default().png_set_add_alpha_channel(true);
    let mut decoder = zune_png::PngDecoder::new_with_options(
        zune_png::zune_core::bytestream::ZCursor::new(bytes),
        options,
    );
    let pixels = decoder.decode().map_err(|e| e.to_string())?;
    let info = decoder.info().ok_or("png: no info")?;
    let width = info.width;
    let height = info.height;
    let rgba = match pixels {
        zune_png::zune_core::result::DecodingResult::U8(data) => data,
        zune_png::zune_core::result::DecodingResult::U16(data) => {
            data.iter().map(|&v| (v >> 8) as u8).collect()
        }
        zune_png::zune_core::result::DecodingResult::F32(data) => data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect(),
        _ => unreachable!("DecodingResult is exhaustive"),
    };
    let out = match decoder.colorspace() {
        Some(zune_png::zune_core::colorspace::ColorSpace::RGBA) => rgba,
        Some(zune_png::zune_core::colorspace::ColorSpace::LumaA) => {
            let mut out = Vec::with_capacity(rgba.len() * 2);
            for c in rgba.chunks_exact(2) {
                out.extend_from_slice(&[c[0], c[0], c[0], c[1]]);
            }
            out
        }
        _ => {
            let mut out = Vec::with_capacity(rgba.len() * 2);
            for &c in &rgba {
                out.extend_from_slice(&[c, c, c, 255]);
            }
            out
        }
    };
    Ok((width as u32, height as u32, out))
}

fn jpeg_decoded_zune(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    use zune_png::zune_core::options::DecoderOptions;
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(zune_png::zune_core::colorspace::ColorSpace::RGBA);
    let mut decoder = zune_jpeg::JpegDecoder::new_with_options(
        zune_png::zune_core::bytestream::ZCursor::new(bytes),
        options,
    );
    let data = decoder.decode().map_err(|e| e.to_string())?;
    let info = decoder.info().ok_or("jpeg: no info")?;
    Ok((u32::from(info.width), u32::from(info.height), data))
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut pos = 2usize;
    while pos + 4 <= bytes.len() {
        if bytes[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = bytes[pos + 1];
        if marker == 0xD8 || marker == 0xD9 {
            return None;
        }
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            if pos + 9 <= bytes.len() {
                let height = u16::from_be_bytes([bytes[pos + 5], bytes[pos + 6]]) as u32;
                let width = u16::from_be_bytes([bytes[pos + 7], bytes[pos + 8]]) as u32;
                return Some((width, height));
            }
            return None;
        }
        if marker == 0x01 || matches!(marker, 0xD0..=0xD7) {
            pos += 2;
            continue;
        }
        let len = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]) as usize;
        if len < 2 {
            return None;
        }
        pos += 2 + len;
    }
    None
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 30 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    if bytes.len() >= 34 && &bytes[12..16] == b"VP8 " {
        let width = u16::from_le_bytes([bytes[26], bytes[27]]) as u32 & 0x3FFF;
        let height = u16::from_le_bytes([bytes[28], bytes[29]]) as u32 & 0x3FFF;
        return Some((width, height));
    }
    if bytes.len() >= 25 && &bytes[12..16] == b"VP8L" {
        let bits = u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
        let width = (bits & 0x3FFF) + 1;
        let height = ((bits >> 14) & 0x3FFF) + 1;
        return Some((width, height));
    }
    None
}

fn decode_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if bytes.len() >= 24 {
            let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
            let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
            return Some((width, height));
        }
        return None;
    }
    if bytes.starts_with(&[0xFF, 0xD8]) {
        return jpeg_dimensions(bytes);
    }
    webp_dimensions(bytes)
}

fn rgb_to_argb_words(rgba: &[u8]) -> Vec<u32> {
    rgba.chunks_exact(4)
        .map(|c| {
            u32::from(c[3]) << 24 | u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2])
        })
        .collect()
}

/// `BitmapFactory.decodeStream(...)`: PNG/JPEG decode fully via zune; WEBP
/// only contributes dimensions (pixels are empty) until a real decoder lands.
pub(crate) fn bitmap_factory_decode_stream(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = match payload(vm, args[1]) {
        Some(Native::ByteArrayInputStream { bytes, pos }) => bytes[*pos..].to_vec(),
        _ => return Err(npe(vm)),
    };
    decode_bitmap_bytes(vm, &bytes)
}

pub(crate) fn bitmap_factory_decode_array(vm: &mut Vm, args: &[JValue]) -> R {
    let bytes = bytes_of(vm, args[1]).ok_or_else(|| npe(vm))?;
    decode_bitmap_bytes(vm, &bytes)
}

pub(crate) fn bitmap_factory_decode_array_opts(vm: &mut Vm, args: &[JValue]) -> R {
    bitmap_factory_decode_array(vm, args)
}

fn decode_bitmap_bytes(vm: &mut Vm, bytes: &[u8]) -> R {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") || bytes.starts_with(&[0xFF, 0xD8]) {
        return match decode_image_rgba(bytes) {
            Ok((width, height, rgba)) => {
                bitmap_alloc(vm, width as i32, height as i32, rgb_to_argb_words(&rgba))
            }
            Err(e) => {
                log::warn!("BitmapFactory: decode failed: {e}");
                Ok(JValue::Null)
            }
        };
    }
    if let Some((width, height)) = decode_dimensions(bytes) {
        log::warn!(
            "BitmapFactory: non-PNG/JPEG decode is dimension-only ({}x{})",
            width,
            height
        );
        return bitmap_alloc(vm, width as i32, height as i32, Vec::new());
    }
    Ok(JValue::Null)
}
fn canvas_bitmap_id(vm: &mut Vm, v: JValue) -> Result<u32, NatErr> {
    match payload(vm, v) {
        Some(Native::Canvas { bitmap, .. }) => Ok(*bitmap),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn canvas_init(vm: &mut Vm, args: &[JValue]) -> R {
    let bitmap = match args[1] {
        JValue::Obj(id) => id,
        _ => return Err(npe(vm)),
    };
    let Some(JValue::Obj(this)) = args.first().copied() else {
        return Err(npe(vm));
    };
    vm.arena.objects[this as usize].native = Some(Native::Canvas {
        bitmap,
        transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        stack: Vec::new(),
    });
    Ok(JValue::Null)
}

pub(crate) fn canvas_draw_bitmap_rect(vm: &mut Vm, args: &[JValue]) -> R {
    let (src_w, src_h, src_px) = bitmap_parts(vm, args[1])?;
    let src = match payload(vm, args[2]) {
        Some(Native::Rect {
            left,
            top,
            right,
            bottom,
        }) => Some((*left, *top, *right, *bottom)),
        None if matches!(args[2], JValue::Null) => None,
        _ => return Err(npe(vm)),
    };
    let dst = match payload(vm, args[3]) {
        Some(Native::Rect {
            left,
            top,
            right,
            bottom,
        }) => Some((*left, *top, *right, *bottom)),
        None if matches!(args[3], JValue::Null) => None,
        _ => return Err(npe(vm)),
    };
    let canvas_id = canvas_bitmap_id(vm, args[0])?;
    let (dst_w, dst_h) = match dst {
        Some((l, t, r, b)) => (r - l, b - t),
        None => match bitmap_parts(vm, JValue::Obj(canvas_id)) {
            Ok((w, h, _)) => (w, h),
            Err(_) => return Err(npe(vm)),
        },
    };
    let (canvas_w, canvas_h, _) = match bitmap_parts(vm, JValue::Obj(canvas_id)) {
        Ok(p) => p,
        Err(_) => return Err(npe(vm)),
    };
    let (sx, sy, sw, sh) = match src {
        Some((l, t, r, b)) => (l, t, r - l, b - t),
        None => (0, 0, src_w, src_h),
    };
    if sw <= 0 || sh <= 0 || dst_w <= 0 || dst_h <= 0 {
        return Ok(JValue::Null);
    }
    let (dst_off_x, dst_off_y) = match dst {
        Some((l, t, _, _)) => (l, t),
        None => (0, 0),
    };
    let mut target = Vec::new();
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let sx_px = sx + sw * dx / dst_w;
            let sy_px = sy + sh * dy / dst_h;
            let idx = (sy_px * src_w + sx_px) as usize;
            let dst_y = dy + dst_off_y;
            let dst_x = dx + dst_off_x;
            if dst_y < 0 || dst_x < 0 || dst_y >= canvas_h || dst_x >= canvas_w {
                continue;
            }
            target.push(((dst_y * canvas_w + dst_x) as usize, src_px[idx]));
        }
    }
    let Native::Bitmap { pixels, .. } = bitmap_mut(vm, JValue::Obj(canvas_id))? else {
        unreachable!("payload checked")
    };
    for (idx, v) in target {
        if let Some(slot) = pixels.get_mut(idx) {
            *slot = v;
        }
    }
    Ok(JValue::Null)
}

pub(crate) fn canvas_draw_bitmap_xy(vm: &mut Vm, args: &[JValue]) -> R {
    let (src_w, src_h, src_px) = bitmap_parts(vm, args[1])?;
    let x = float_of(vm, args[2]);
    let y = float_of(vm, args[3]);
    let canvas_id = canvas_bitmap_id(vm, args[0])?;
    let (dst_w, dst_h, _) = match bitmap_parts(vm, JValue::Obj(canvas_id)) {
        Ok(p) => p,
        Err(_) => return Err(npe(vm)),
    };
    let x0 = x.max(0.0) as i32;
    let y0 = y.max(0.0) as i32;
    let mut target = Vec::new();
    for sy in 0..src_h {
        for sx in 0..src_w {
            let dx = x0 + sx;
            let dy = y0 + sy;
            if dx >= 0 && dy >= 0 && dx < dst_w && dy < dst_h {
                target.push((
                    (dy * dst_w + dx) as usize,
                    src_px[(sy * src_w + sx) as usize],
                ));
            }
        }
    }
    let Native::Bitmap { pixels, .. } = bitmap_mut(vm, JValue::Obj(canvas_id))? else {
        unreachable!("payload checked")
    };
    for (idx, v) in target {
        pixels[idx] = v;
    }
    Ok(JValue::Null)
}

pub(crate) fn canvas_draw_color(vm: &mut Vm, args: &[JValue]) -> R {
    let color = int_of(vm, args[1]) as u32;
    let canvas_id = canvas_bitmap_id(vm, args[0])?;
    let (width, height, _) = bitmap_parts(vm, JValue::Obj(canvas_id))?;
    let mut target = raqote::DrawTarget::new(width, height);
    let rgba = raqote::SolidSource {
        r: (color >> 16) as u8,
        g: (color >> 8) as u8,
        b: color as u8,
        a: (color >> 24) as u8,
    };
    target.clear(rgba);
    let Native::Bitmap { pixels, .. } = bitmap_mut(vm, JValue::Obj(canvas_id))? else {
        unreachable!()
    };
    pixels.copy_from_slice(target.get_data());
    Ok(JValue::Null)
}

pub(crate) fn canvas_noop(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn canvas_save(_vm: &mut Vm, _args: &[JValue]) -> R {
    let vm = _vm;
    let Some(Native::Canvas {
        transform, stack, ..
    }) = payload_mut(vm, _args[0])
    else {
        return Err(npe(vm));
    };
    stack.push(*transform);
    Ok(JValue::Int(stack.len() as i32))
}

pub(crate) fn canvas_restore(vm: &mut Vm, args: &[JValue]) -> R {
    let Some(Native::Canvas {
        transform, stack, ..
    }) = payload_mut(vm, args[0])
    else {
        return Err(npe(vm));
    };
    if let Some(saved) = stack.pop() {
        *transform = saved;
    }
    Ok(JValue::Null)
}

pub(crate) fn canvas_translate(vm: &mut Vm, args: &[JValue]) -> R {
    let dx = float_of(vm, args[1]);
    let dy = float_of(vm, args[2]);
    let Some(Native::Canvas { transform, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    transform[4] += transform[0] * dx + transform[2] * dy;
    transform[5] += transform[1] * dx + transform[3] * dy;
    Ok(JValue::Null)
}

pub(crate) fn canvas_rotate(vm: &mut Vm, args: &[JValue]) -> R {
    let radians = float_of(vm, args[1]).to_radians();
    let (sin, cos) = radians.sin_cos();
    let Some(Native::Canvas { transform, .. }) = payload_mut(vm, args[0]) else {
        return Err(npe(vm));
    };
    let [a, b, c, d, tx, ty] = *transform;
    *transform = [
        a * cos + c * sin,
        b * cos + d * sin,
        c * cos - a * sin,
        d * cos - b * sin,
        tx,
        ty,
    ];
    Ok(JValue::Null)
}

fn paint_mut(vm: &mut Vm, v: JValue) -> Result<&mut Native, NatErr> {
    if !matches!(payload(vm, v), Some(Native::Paint { .. })) {
        return Err(npe(vm));
    }
    Ok(payload_mut(vm, v).expect("payload checked"))
}

pub(crate) fn paint_init(vm: &mut Vm, args: &[JValue]) -> R {
    let _ = paint_mut(vm, args[0])?;
    Ok(JValue::Null)
}

pub(crate) fn paint_set_color(vm: &mut Vm, args: &[JValue]) -> R {
    let color = int_of(vm, args[1]);
    let Native::Paint {
        color: slot,
        text_size: _,
        stroke_width: _,
        style: _,
    } = paint_mut(vm, args[0])?
    else {
        unreachable!("payload checked")
    };
    *slot = color;
    Ok(JValue::Null)
}

pub(crate) fn paint_set_text_size(vm: &mut Vm, args: &[JValue]) -> R {
    let size = float_of(vm, args[1]);
    let Native::Paint {
        color: _,
        text_size: slot,
        stroke_width: _,
        style: _,
    } = paint_mut(vm, args[0])?
    else {
        unreachable!("payload checked")
    };
    *slot = size;
    Ok(JValue::Null)
}

pub(crate) fn paint_set_stroke_width(vm: &mut Vm, args: &[JValue]) -> R {
    let width = float_of(vm, args[1]);
    let Native::Paint {
        color: _,
        text_size: _,
        stroke_width: slot,
        style: _,
    } = paint_mut(vm, args[0])?
    else {
        unreachable!("payload checked")
    };
    *slot = width;
    Ok(JValue::Null)
}

pub(crate) fn paint_set_style(vm: &mut Vm, args: &[JValue]) -> R {
    let style = int_of(vm, args[1]);
    let Native::Paint {
        color: _,
        text_size: _,
        stroke_width: _,
        style: slot,
    } = paint_mut(vm, args[0])?
    else {
        unreachable!("payload checked")
    };
    *slot = style;
    Ok(JValue::Null)
}

pub(crate) fn paint_set_antialias(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn paint_set_typeface(_vm: &mut Vm, args: &[JValue]) -> R {
    Ok(args[2])
}

pub(crate) fn paint_get_color(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Paint { color, .. }) => Ok(JValue::Int(*color)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn paint_get_style(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Int(0))
}

pub(crate) fn paint_get_text_size(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Paint { text_size, .. }) => Ok(JValue::Float(*text_size)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn paint_get_font_metrics(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/graphics/Paint$FontMetrics;", Native::Opaque)
}

fn rect_mut(vm: &mut Vm, v: JValue) -> Result<&mut Native, NatErr> {
    if !matches!(payload(vm, v), Some(Native::Rect { .. })) {
        return Err(npe(vm));
    }
    Ok(payload_mut(vm, v).expect("payload checked"))
}

pub(crate) fn rect_init(vm: &mut Vm, args: &[JValue]) -> R {
    let _ = rect_mut(vm, args[0])?;
    Ok(JValue::Null)
}

pub(crate) fn rect_init_4(vm: &mut Vm, args: &[JValue]) -> R {
    let (l, t, r, b) = (
        int_of(vm, args[1]),
        int_of(vm, args[2]),
        int_of(vm, args[3]),
        int_of(vm, args[4]),
    );
    let Native::Rect {
        left,
        top,
        right,
        bottom,
    } = rect_mut(vm, args[0])?
    else {
        unreachable!("payload checked")
    };
    *left = l;
    *top = t;
    *right = r;
    *bottom = b;
    Ok(JValue::Null)
}

pub(crate) fn rect_set(vm: &mut Vm, args: &[JValue]) -> R {
    rect_init_4(vm, args)
}

pub(crate) fn rect_width(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Rect { left, right, .. }) => Ok(JValue::Int(*right - *left)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn rect_height(vm: &mut Vm, args: &[JValue]) -> R {
    match payload(vm, args[0]) {
        Some(Native::Rect { top, bottom, .. }) => Ok(JValue::Int(*bottom - *top)),
        _ => Err(npe(vm)),
    }
}

pub(crate) fn static_layout_init(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn layout_draw(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn static_layout_builder_obtain(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/text/StaticLayout$Builder;", Native::Opaque)
}

pub(crate) fn static_layout_builder_build(vm: &mut Vm, _args: &[JValue]) -> R {
    alloc(vm, "Landroid/text/StaticLayout;", Native::Opaque)
}

pub(crate) fn pdf_page_render(_vm: &mut Vm, _args: &[JValue]) -> R {
    Ok(JValue::Null)
}

pub(crate) fn image_decoder_decode(vm: &mut Vm, _args: &[JValue]) -> R {
    bitmap_alloc(vm, 0, 0, Vec::new())
}

pub(crate) const GRAPHICS_TABLE: &[NativeEntry] = &[
    ne!(
        "Landroid/graphics/Bitmap;",
        "getWidth",
        "()I",
        true,
        bitmap_get_width
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "getHeight",
        "()I",
        true,
        bitmap_get_height
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "getPixel",
        "(II)I",
        true,
        bitmap_get_pixel
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "getPixels",
        "([IIIIIII)V",
        true,
        bitmap_get_pixels
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "setPixels",
        "([IIIIIII)V",
        true,
        bitmap_set_pixels
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "recycle",
        "()V",
        true,
        bitmap_recycle
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "eraseColor",
        "(I)V",
        true,
        bitmap_erase_color
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "copy",
        "(Landroid/graphics/Bitmap$Config;Z)Landroid/graphics/Bitmap;",
        true,
        bitmap_copy
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "createBitmap",
        "(IILandroid/graphics/Bitmap$Config;)Landroid/graphics/Bitmap;",
        false,
        bitmap_create_empty
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "createBitmap",
        "(Landroid/graphics/Bitmap;IIII)Landroid/graphics/Bitmap;",
        false,
        bitmap_crop
    ),
    ne!(
        "Landroid/graphics/Bitmap;",
        "compress",
        "(Landroid/graphics/Bitmap$CompressFormat;ILjava/io/OutputStream;)Z",
        true,
        bitmap_compress
    ),
    ne!(
        "Landroid/graphics/BitmapFactory;",
        "decodeStream",
        "(Ljava/io/InputStream;)Landroid/graphics/Bitmap;",
        false,
        bitmap_factory_decode_stream
    ),
    ne!(
        "Landroid/graphics/BitmapFactory;",
        "decodeByteArray",
        "([BII)Landroid/graphics/Bitmap;",
        false,
        bitmap_factory_decode_array
    ),
    ne!(
        "Landroid/graphics/BitmapFactory;",
        "decodeByteArray",
        "([BIILandroid/graphics/BitmapFactory$Options;)Landroid/graphics/Bitmap;",
        false,
        bitmap_factory_decode_array_opts
    ),
    ne!(
        "Landroid/graphics/BitmapFactory$Options;",
        "<init>",
        "()V",
        true,
        canvas_noop
    ),
    ne!(
        "Landroid/graphics/Canvas;",
        "<init>",
        "(Landroid/graphics/Bitmap;)V",
        true,
        canvas_init
    ),
    ne!(
        "Landroid/graphics/Canvas;",
        "drawBitmap",
        "(Landroid/graphics/Bitmap;Landroid/graphics/Rect;Landroid/graphics/Rect;Landroid/graphics/Paint;)V",
        true,
        canvas_draw_bitmap_rect
    ),
    ne!(
        "Landroid/graphics/Canvas;",
        "drawBitmap",
        "(Landroid/graphics/Bitmap;FFLandroid/graphics/Paint;)V",
        true,
        canvas_draw_bitmap_xy
    ),
    ne!(
        "Landroid/graphics/Canvas;",
        "drawColor",
        "(I)V",
        true,
        canvas_draw_color
    ),
    ne!(
        "Landroid/graphics/Canvas;",
        "save",
        "()I",
        true,
        canvas_save
    ),
    ne!("Landroid/graphics/Canvas;", "restore", "()V", true, canvas_restore),
    ne!(
        "Landroid/graphics/Canvas;",
        "translate",
        "(FF)V",
        true,
        canvas_translate
    ),
    ne!("Landroid/graphics/Canvas;", "rotate", "(F)V", true, canvas_rotate),
    ne!("Landroid/graphics/Paint;", "<init>", "()V", true, paint_init),
    ne!(
        "Landroid/graphics/Paint;",
        "setColor",
        "(I)V",
        true,
        paint_set_color
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "setTextSize",
        "(F)V",
        true,
        paint_set_text_size
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "setStrokeWidth",
        "(F)V",
        true,
        paint_set_stroke_width
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "setStyle",
        "(Landroid/graphics/Paint$Style;)V",
        true,
        paint_set_style
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "setAntiAlias",
        "(Z)V",
        true,
        paint_set_antialias
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "setTypeface",
        "(Landroid/graphics/Typeface;)Landroid/graphics/Typeface;",
        true,
        paint_set_typeface
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "getColor",
        "()I",
        true,
        paint_get_color
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "getStyle",
        "()Landroid/graphics/Paint$Style;",
        true,
        paint_get_style
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "getTextSize",
        "()F",
        true,
        paint_get_text_size
    ),
    ne!(
        "Landroid/graphics/Paint;",
        "getFontMetrics",
        "()Landroid/graphics/Paint$FontMetrics;",
        true,
        paint_get_font_metrics
    ),
    ne!("Landroid/graphics/Rect;", "<init>", "()V", true, rect_init),
    ne!(
        "Landroid/graphics/Rect;",
        "<init>",
        "(IIII)V",
        true,
        rect_init_4
    ),
    ne!(
        "Landroid/graphics/Rect;",
        "set",
        "(IIII)V",
        true,
        rect_set
    ),
    ne!("Landroid/graphics/Rect;", "width", "()I", true, rect_width),
    ne!("Landroid/graphics/Rect;", "height", "()I", true, rect_height),
    ne!(
        "Landroid/graphics/ImageDecoder;",
        "decodeBitmap",
        "(Landroid/graphics/ImageDecoder$Source;Landroid/graphics/ImageDecoder$OnHeaderDecodedListener;)Landroid/graphics/Bitmap;",
        false,
        image_decoder_decode
    ),
    ne!(
        "Landroid/graphics/pdf/PdfRenderer$Page;",
        "<init>",
        "()V",
        true,
        canvas_rotate
    ),
    ne!(
        "Landroid/graphics/pdf/PdfRenderer$Page;",
        "render",
        "(Landroid/graphics/Bitmap;Landroid/graphics/Rect;Landroid/graphics/Matrix;I)V",
        true,
        pdf_page_render
    ),
    ne!(
        "Landroid/text/TextPaint;",
        "<init>",
        "()V",
        true,
        paint_init
    ),
    ne!(
        "Landroid/text/StaticLayout;",
        "<init>",
        "(Landroid/text/CharSequence;Landroid/text/TextPaint;ILandroid/text/Layout$Alignment;FFZ)V",
        true,
        static_layout_init
    ),
    ne!(
        "Landroid/text/StaticLayout$Builder;",
        "obtain",
        "(Landroid/text/CharSequence;IILandroid/text/TextPaint;I)Landroid/text/StaticLayout$Builder;",
        false,
        static_layout_builder_obtain
    ),
    ne!(
        "Landroid/text/StaticLayout$Builder;",
        "build",
        "()Landroid/text/StaticLayout;",
        true,
        static_layout_builder_build
    ),
    ne!(
        "Landroid/text/Layout;",
        "draw",
        "(Landroid/graphics/Canvas;)V",
        true,
        layout_draw
    ),
];
