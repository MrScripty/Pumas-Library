//! Bounded still-image validation and projection; no URL/path acquisition.
//!
//! Dimensions/pixel/source limits are checked before raster decode. The codec's
//! 64 MiB allocation limit is best effort, not process/OS memory containment.
use super::types::{ErrorCode, ImageEncoding, ImagePart, OperationInput, Role, MAX_BYTES};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{codecs::png::PngDecoder, DynamicImage, ImageDecoder, Limits};
use serde_json::{json, Value};
use std::io::Cursor;

const MAX_IMAGE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_IMAGES: usize = 4;
const MAX_MESSAGES: usize = 128;
const MAX_PARTS: usize = 128;
const MAX_ENCODED_BYTES: usize = MAX_IMAGE_BYTES.div_ceil(3) * 4;
const MAX_TOTAL_ENCODED_BYTES: usize = MAX_TOTAL_IMAGE_BYTES.div_ceil(3) * 4 + MAX_IMAGES * 4;
const MAX_DIMENSION: u32 = 4096;
const MAX_PIXELS: u64 = 4_194_304;
const MAX_ALLOC: u64 = 64 * 1024 * 1024;
const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

#[derive(Default)]
struct Budget {
    images: usize,
    encoded: usize,
    source: usize,
    parts: usize,
    text: usize,
}
impl Budget {
    fn image(&mut self, encoded: usize) -> Result<(), ErrorCode> {
        self.images += 1;
        self.encoded += encoded;
        if self.images > MAX_IMAGES
            || encoded > MAX_ENCODED_BYTES
            || self.encoded > MAX_TOTAL_ENCODED_BYTES
        {
            return Err(ErrorCode::RequestLimit);
        }
        Ok(())
    }
    fn source(&mut self, bytes: usize) -> Result<(), ErrorCode> {
        self.source += bytes;
        if bytes > MAX_IMAGE_BYTES || self.source > MAX_TOTAL_IMAGE_BYTES {
            return Err(ErrorCode::RequestLimit);
        }
        Ok(())
    }
    fn part(&mut self) -> Result<(), ErrorCode> {
        self.parts += 1;
        if self.parts > MAX_PARTS {
            return Err(ErrorCode::RequestLimit);
        }
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<(), ErrorCode> {
        if text.trim().is_empty() {
            return Err(ErrorCode::InvalidRequest);
        }
        self.text += text.len();
        if self.text > MAX_BYTES {
            return Err(ErrorCode::RequestLimit);
        }
        Ok(())
    }
}

pub(super) fn provider_messages(input: &OperationInput) -> Result<Value, ErrorCode> {
    let mut budget = Budget::default();
    match input {
        OperationInput::Image {
            encoding,
            data_base64,
        } => {
            let image = image_part(*encoding, data_base64, &mut budget)?;
            Ok(json!([{"role":"user", "content":[
                {"type":"text", "text":"Describe this image."}, image
            ]}]))
        }
        OperationInput::ImageMessages { messages } => {
            if messages.is_empty() {
                return Err(ErrorCode::InvalidRequest);
            }
            if messages.len() > MAX_MESSAGES {
                return Err(ErrorCode::RequestLimit);
            }
            let mut output = Vec::with_capacity(messages.len());
            let mut user_image = false;
            for message in messages {
                if message.content.is_empty() {
                    return Err(ErrorCode::InvalidRequest);
                }
                if message.content.len() > MAX_PARTS {
                    return Err(ErrorCode::RequestLimit);
                }
                let mut content = Vec::with_capacity(message.content.len());
                for part in &message.content {
                    budget.part()?;
                    content.push(match part {
                        ImagePart::Text { text } => {
                            budget.text(text)?;
                            json!({"type":"text", "text":text})
                        }
                        ImagePart::Image {
                            encoding,
                            data_base64,
                        } => {
                            if !matches!(message.role, Role::User) {
                                return Err(ErrorCode::InvalidRequest);
                            }
                            user_image = true;
                            image_part(*encoding, data_base64, &mut budget)?
                        }
                    });
                }
                let role = match message.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                output.push(json!({"role":role, "content":content}));
            }
            if !user_image {
                return Err(ErrorCode::InvalidRequest);
            }
            Ok(Value::Array(output))
        }
        _ => Err(ErrorCode::InvalidRequest),
    }
}

fn image_part(
    encoding: ImageEncoding,
    encoded: &str,
    budget: &mut Budget,
) -> Result<Value, ErrorCode> {
    budget.image(encoded.len())?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| ErrorCode::InvalidRequest)?;
    if bytes.is_empty() {
        return Err(ErrorCode::InvalidRequest);
    }
    budget.source(bytes.len())?;
    // Standard alphabet/padding and zero pad bits are mandatory; URLs, data-URL
    // envelopes, whitespace and noncanonical encodings are not inputs.
    let canonical = STANDARD.encode(&bytes);
    if canonical != encoded {
        return Err(ErrorCode::InvalidRequest);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_ALLOC);
    let mime = match encoding {
        ImageEncoding::Png => {
            png_container(&bytes)?;
            let decoder =
                PngDecoder::with_limits(Cursor::new(&bytes), limits).map_err(codec_error)?;
            decode(decoder)?;
            "image/png"
        }
        ImageEncoding::Jpeg => {
            jpeg_container(&bytes)?;
            strict_jpeg(&bytes)?;
            "image/jpeg"
        }
    };
    // Preserve the validated compressed source bytes; never rewrite image content.
    Ok(json!({"type":"image_url", "image_url":{"url":format!("data:{mime};base64,{canonical}")}}))
}

fn codec_error(error: image::ImageError) -> ErrorCode {
    if matches!(error, image::ImageError::Limits(_)) {
        ErrorCode::RequestLimit
    } else {
        ErrorCode::InvalidRequest
    }
}
fn dimensions(width: u32, height: u32) -> Result<(), ErrorCode> {
    if width == 0 || height == 0 {
        return Err(ErrorCode::InvalidRequest);
    }
    if width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(ErrorCode::RequestLimit);
    }
    Ok(())
}
fn decode(decoder: impl ImageDecoder) -> Result<(), ErrorCode> {
    let (width, height) = decoder.dimensions();
    dimensions(width, height)?;
    DynamicImage::from_decoder(decoder).map_err(codec_error)?;
    Ok(())
}

fn strict_jpeg(bytes: &[u8]) -> Result<Vec<u8>, ErrorCode> {
    // The safe TurboJPEG wrapper returns errors for warnings as well as fatal
    // errors. In particular, incomplete MCU entropy cannot be accepted through
    // libjpeg's zero-bit recovery. Container framing alone cannot prove this.
    let mut decoder = turbojpeg::Decompressor::new().map_err(|_| ErrorCode::InvalidRequest)?;
    let header = decoder
        .read_header(bytes)
        .map_err(|_| ErrorCode::InvalidRequest)?;
    dimensions(
        u32::try_from(header.width).map_err(|_| ErrorCode::RequestLimit)?,
        u32::try_from(header.height).map_err(|_| ErrorCode::RequestLimit)?,
    )?;
    let pitch = header.width.checked_mul(3).ok_or(ErrorCode::RequestLimit)?;
    let output_size = pitch
        .checked_mul(header.height)
        .filter(|size| *size <= MAX_ALLOC as usize)
        .ok_or(ErrorCode::RequestLimit)?;
    // This bound covers the RGB output raster; native codec scratch allocations
    // remain codec-managed, not a hard 64 MiB total allocation budget.
    let mut pixels = vec![0; output_size];
    decoder
        .decompress(
            bytes,
            turbojpeg::Image {
                pixels: pixels.as_mut_slice(),
                width: header.width,
                pitch,
                height: header.height,
                format: turbojpeg::PixelFormat::RGB,
            },
        )
        .map_err(|_| ErrorCode::InvalidRequest)?;
    Ok(pixels)
}

// PNG container framing/CRCs supplement full raster decode: reject APNG and a
// truncated/missing IEND or extra trailing payload, including ignored metadata.
fn png_container(bytes: &[u8]) -> Result<(), ErrorCode> {
    if !bytes.starts_with(PNG_MAGIC) {
        return Err(ErrorCode::InvalidRequest);
    }
    let mut offset = PNG_MAGIC.len();
    let mut header = false;
    while offset < bytes.len() {
        let prefix = bytes
            .get(offset..offset + 8)
            .ok_or(ErrorCode::InvalidRequest)?;
        let length = u32::from_be_bytes(
            prefix[..4]
                .try_into()
                .map_err(|_| ErrorCode::InvalidRequest)?,
        ) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .filter(|n| *n <= bytes.len())
            .ok_or(ErrorCode::InvalidRequest)?;
        let kind = &prefix[4..8];
        if matches!(kind, b"acTL" | b"fcTL" | b"fdAT") {
            return Err(ErrorCode::InvalidRequest);
        }
        let crc = u32::from_be_bytes(
            bytes[end - 4..end]
                .try_into()
                .map_err(|_| ErrorCode::InvalidRequest)?,
        );
        if crc32(&bytes[offset + 4..end - 4]) != crc {
            return Err(ErrorCode::InvalidRequest);
        }
        if !header {
            if kind != b"IHDR" || length != 13 {
                return Err(ErrorCode::InvalidRequest);
            }
            let data = &bytes[offset + 8..end - 4];
            dimensions(
                u32::from_be_bytes(
                    data[..4]
                        .try_into()
                        .map_err(|_| ErrorCode::InvalidRequest)?,
                ),
                u32::from_be_bytes(
                    data[4..8]
                        .try_into()
                        .map_err(|_| ErrorCode::InvalidRequest)?,
                ),
            )?;
            header = true;
        } else if kind == b"IHDR" {
            return Err(ErrorCode::InvalidRequest);
        }
        if kind == b"IEND" {
            return if length == 0 && end == bytes.len() {
                Ok(())
            } else {
                Err(ErrorCode::InvalidRequest)
            };
        }
        offset = end;
    }
    Err(ErrorCode::InvalidRequest)
}

fn crc32(bytes: &[u8]) -> u32 {
    crc32fast::hash(bytes)
}

// Require a complete single JPEG marker stream, not a decoder-recovered missing
// end marker. The codec remains authoritative for tables and raster/entropy data.
fn jpeg_container(bytes: &[u8]) -> Result<(), ErrorCode> {
    if !bytes.starts_with(b"\xff\xd8") {
        return Err(ErrorCode::InvalidRequest);
    }
    let mut offset = 2;
    let mut scan = false;
    let mut saw_scan = false;
    let mut frame = false;
    while offset < bytes.len() {
        if scan && bytes[offset] != 0xff {
            offset += 1;
            continue;
        }
        if bytes[offset] != 0xff {
            return Err(ErrorCode::InvalidRequest);
        }
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *bytes.get(offset).ok_or(ErrorCode::InvalidRequest)?;
        offset += 1;
        if scan && (marker == 0 || (0xd0..=0xd7).contains(&marker)) {
            continue;
        }
        scan = false;
        if marker == 0xd9 {
            return if frame && saw_scan && offset == bytes.len() {
                Ok(())
            } else {
                Err(ErrorCode::InvalidRequest)
            };
        }
        if marker == 0 || marker == 0xd8 || (0xd0..=0xd7).contains(&marker) {
            return Err(ErrorCode::InvalidRequest);
        }
        let length = bytes
            .get(offset..offset + 2)
            .ok_or(ErrorCode::InvalidRequest)?;
        let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
        let end = offset
            .checked_add(length)
            .filter(|n| length >= 2 && *n <= bytes.len())
            .ok_or(ErrorCode::InvalidRequest)?;
        let data = &bytes[offset + 2..end];
        if marker == 0xe2 && data.starts_with(b"MPF\0") {
            return Err(ErrorCode::InvalidRequest);
        }
        if (0xc0..=0xcf).contains(&marker) && !matches!(marker, 0xc4 | 0xc8 | 0xcc) {
            if frame || data.len() < 6 || data.len() != 6 + 3 * usize::from(data[5]) {
                return Err(ErrorCode::InvalidRequest);
            }
            dimensions(
                u32::from(u16::from_be_bytes([data[3], data[4]])),
                u32::from(u16::from_be_bytes([data[1], data[2]])),
            )?;
            frame = true;
        }
        if marker == 0xda {
            if !frame || data.is_empty() || data.len() != 4 + 2 * usize::from(data[0]) {
                return Err(ErrorCode::InvalidRequest);
            }
            saw_scan = true;
            scan = true;
        }
        offset = end;
    }
    Err(ErrorCode::InvalidRequest)
}

#[cfg(test)]
mod tests {
    use super::super::types::ImageMessage;
    use super::*;
    const PNG: &[u8] = include_bytes!("../../../tests/fixtures/image-text/red-blue.png");
    const JPEG: &[u8] = include_bytes!("../../../tests/fixtures/image-text/red-blue.jpg");
    const APNG: &[u8] = include_bytes!("../../../tests/fixtures/image-text/animated-red-blue.png");
    const PROGRESSIVE: &[u8] =
        include_bytes!("../../../tests/fixtures/image-text/progressive-red-blue.jpg");
    fn input(encoding: ImageEncoding, bytes: &[u8]) -> OperationInput {
        OperationInput::Image {
            encoding,
            data_base64: STANDARD.encode(bytes),
        }
    }
    fn image(encoding: ImageEncoding, bytes: &[u8]) -> ImagePart {
        ImagePart::Image {
            encoding,
            data_base64: STANDARD.encode(bytes),
        }
    }
    #[test]
    fn real_png_jpeg_pixels_and_original_bytes_are_preserved() {
        for (encoding, bytes, mime) in [
            (ImageEncoding::Png, PNG, "image/png"),
            (ImageEncoding::Jpeg, JPEG, "image/jpeg"),
        ] {
            let pixels = match encoding {
                ImageEncoding::Png => {
                    let decoded = image::load_from_memory(bytes).unwrap().to_rgb8();
                    assert_eq!(decoded.dimensions(), (2, 1));
                    decoded.into_raw()
                }
                ImageEncoding::Jpeg => strict_jpeg(bytes).unwrap(),
            };
            assert_eq!(pixels.len(), 6);
            let red = &pixels[..3];
            let blue = &pixels[3..];
            assert!(red[0] >= 250 && red[1] <= 5 && red[2] <= 5);
            assert!(blue[0] <= 5 && blue[1] <= 5 && blue[2] >= 250);
            let result = provider_messages(&input(encoding, bytes)).unwrap();
            assert_eq!(result[0]["content"][0]["text"], "Describe this image.");
            assert_eq!(
                result[0]["content"][1]["image_url"]["url"],
                format!("data:{mime};base64,{}", STANDARD.encode(bytes))
            );
        }
    }
    #[test]
    fn ordered_mixed_text_roles_and_image_parts_are_preserved() {
        let messages = vec![
            ImageMessage {
                role: Role::System,
                content: vec![ImagePart::Text {
                    text: "Answer briefly.".into(),
                }],
            },
            ImageMessage {
                role: Role::User,
                content: vec![
                    ImagePart::Text {
                        text: "Compare: ".into(),
                    },
                    image(ImageEncoding::Png, PNG),
                    ImagePart::Text {
                        text: " then ".into(),
                    },
                    image(ImageEncoding::Jpeg, JPEG),
                ],
            },
            ImageMessage {
                role: Role::Assistant,
                content: vec![ImagePart::Text {
                    text: "Earlier answer".into(),
                }],
            },
        ];
        let result = provider_messages(&OperationInput::ImageMessages { messages }).unwrap();
        assert_eq!(result[0]["role"], "system");
        assert_eq!(result[1]["content"].as_array().unwrap().len(), 4);
        assert_eq!(result[1]["content"][2]["text"], " then ");
        assert_eq!(result[2]["role"], "assistant");
    }
    #[test]
    fn corruption_truncation_apng_and_mismatched_magic_are_refused() {
        let mut corrupt = PNG.to_vec();
        corrupt[20] ^= 1;
        for request in [
            input(ImageEncoding::Png, &corrupt),
            input(ImageEncoding::Png, &PNG[..PNG.len() - 1]),
            input(ImageEncoding::Jpeg, &JPEG[..JPEG.len() - 2]),
            input(ImageEncoding::Png, APNG),
            input(ImageEncoding::Png, JPEG),
            input(ImageEncoding::Jpeg, PNG),
            input(ImageEncoding::Png, b"GIF89a"),
            input(ImageEncoding::Jpeg, b"\xff\xd8\xff\xd9"),
        ] {
            assert_eq!(provider_messages(&request), Err(ErrorCode::InvalidRequest));
        }
    }
    #[test]
    fn corrupt_raster_with_correct_png_crc_is_refused_by_codec() {
        let mut bytes = PNG.to_vec();
        let offset = bytes.windows(4).position(|x| x == b"IDAT").unwrap();
        let len = u32::from_be_bytes(bytes[offset - 4..offset].try_into().unwrap()) as usize;
        bytes[offset + 4] ^= 1;
        let crc = crc32(&bytes[offset..offset + 4 + len]);
        bytes[offset + 4 + len..offset + 8 + len].copy_from_slice(&crc.to_be_bytes());
        assert_eq!(
            provider_messages(&input(ImageEncoding::Png, &bytes)),
            Err(ErrorCode::InvalidRequest)
        );
    }
    #[test]
    fn canonical_padding_alphabet_and_data_urls_are_required() {
        let original = STANDARD.encode(PNG);
        assert!(original.ends_with("=="));
        let mut nonzero_padding = original.as_bytes().to_vec();
        // For two pads, the final data symbol's lower four bits must be zero.
        let index = nonzero_padding.len() - 3;
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let symbol = alphabet
            .iter()
            .position(|x| *x == nonzero_padding[index])
            .unwrap();
        nonzero_padding[index] = alphabet[symbol + 1];
        for data in [
            "".into(),
            format!("{original}\n"),
            original.trim_end_matches('=').into(),
            format!("data:image/png;base64,{original}"),
            "____".into(),
            String::from_utf8(nonzero_padding).unwrap(),
        ] {
            assert_eq!(
                provider_messages(&OperationInput::Image {
                    encoding: ImageEncoding::Png,
                    data_base64: data
                }),
                Err(ErrorCode::InvalidRequest)
            );
        }
    }
    #[test]
    fn png_dimensions_and_pixel_bombs_refuse_before_raster_decode() {
        for (w, h) in [(4097_u32, 1_u32), (1, 4097), (4096, 4096), (2049, 2048)] {
            let mut bytes = PNG.to_vec();
            bytes[16..20].copy_from_slice(&w.to_be_bytes());
            bytes[20..24].copy_from_slice(&h.to_be_bytes());
            let crc = crc32(&bytes[12..29]);
            bytes[29..33].copy_from_slice(&crc.to_be_bytes());
            assert_eq!(
                provider_messages(&input(ImageEncoding::Png, &bytes)),
                Err(ErrorCode::RequestLimit)
            );
        }
    }
    #[test]
    fn user_image_is_required_and_other_role_images_are_refused() {
        for role in [Role::System, Role::Assistant] {
            assert_eq!(
                provider_messages(&OperationInput::ImageMessages {
                    messages: vec![ImageMessage {
                        role,
                        content: vec![image(ImageEncoding::Png, PNG)]
                    }]
                }),
                Err(ErrorCode::InvalidRequest)
            );
        }
        assert_eq!(
            provider_messages(&OperationInput::ImageMessages {
                messages: vec![ImageMessage {
                    role: Role::User,
                    content: vec![ImagePart::Text {
                        text: "No image".into()
                    }]
                }]
            }),
            Err(ErrorCode::InvalidRequest)
        );
    }
    #[test]
    fn image_count_and_total_part_count_are_bounded() {
        let request = |content| OperationInput::ImageMessages {
            messages: vec![ImageMessage {
                role: Role::User,
                content,
            }],
        };
        assert!(provider_messages(&request(vec![image(ImageEncoding::Png, PNG); 4])).is_ok());
        assert_eq!(
            provider_messages(&request(vec![image(ImageEncoding::Png, PNG); 5])),
            Err(ErrorCode::RequestLimit)
        );
        let mut content = vec![ImagePart::Text { text: "a".into() }; 128];
        content.push(image(ImageEncoding::Png, PNG));
        assert_eq!(
            provider_messages(&request(content)),
            Err(ErrorCode::RequestLimit)
        );
    }
    #[test]
    fn source_and_encoded_limits_have_exact_boundaries() {
        let mut budget = Budget::default();
        assert_eq!(budget.source(MAX_IMAGE_BYTES), Ok(()));
        assert_eq!(budget.source(MAX_IMAGE_BYTES), Ok(()));
        assert_eq!(budget.source(1), Err(ErrorCode::RequestLimit));
        assert_eq!(
            Budget::default().source(MAX_IMAGE_BYTES + 1),
            Err(ErrorCode::RequestLimit)
        );
        assert_eq!(
            Budget::default().image(MAX_ENCODED_BYTES + 1),
            Err(ErrorCode::RequestLimit)
        );
        assert_eq!(MAX_ENCODED_BYTES, 11_184_812);
        let mut encoded_budget = Budget::default();
        assert_eq!(encoded_budget.image(MAX_ENCODED_BYTES), Ok(()));
        assert_eq!(encoded_budget.image(MAX_ENCODED_BYTES), Ok(()));
        assert_eq!(encoded_budget.image(32), Err(ErrorCode::RequestLimit));
    }
    #[test]
    fn strict_jpeg_accepts_progressive_and_refuses_truncated_entropy() {
        assert!(provider_messages(&input(ImageEncoding::Jpeg, PROGRESSIVE)).is_ok());
        let mut truncated = JPEG[..JPEG.len() - 12].to_vec();
        truncated.extend_from_slice(b"\xff\xd9");
        assert_eq!(
            provider_messages(&input(ImageEncoding::Jpeg, &truncated)),
            Err(ErrorCode::InvalidRequest)
        );
    }
    #[test]
    fn jpeg_entropy_cuts_with_complete_container_are_refused_by_codec() {
        // Keep valid SOS/EOI framing: these cut the baseline fixture's entropy,
        // so the container checker passes and the raster decoder must refuse.
        for cut in [12, 18, 24, 32] {
            let mut truncated = JPEG[..JPEG.len() - cut].to_vec();
            truncated.extend_from_slice(b"\xff\xd9");
            assert_eq!(jpeg_container(&truncated), Ok(()));
            assert_eq!(strict_jpeg(&truncated), Err(ErrorCode::InvalidRequest));
            assert_eq!(
                provider_messages(&input(ImageEncoding::Jpeg, &truncated)),
                Err(ErrorCode::InvalidRequest)
            );
        }
    }
    #[test]
    fn jpeg_dimension_and_pixel_bombs_refuse_before_raster_decode() {
        let frame = JPEG.windows(2).position(|x| x == b"\xff\xc0").unwrap();
        for (width, height) in [(4097_u16, 1_u16), (4096, 4096)] {
            let mut bytes = JPEG.to_vec();
            bytes[frame + 5..frame + 7].copy_from_slice(&height.to_be_bytes());
            bytes[frame + 7..frame + 9].copy_from_slice(&width.to_be_bytes());
            assert_eq!(
                provider_messages(&input(ImageEncoding::Jpeg, &bytes)),
                Err(ErrorCode::RequestLimit)
            );
        }
    }
    #[test]
    fn message_and_total_part_limits_include_text_only_messages() {
        let text = || ImageMessage {
            role: Role::System,
            content: vec![ImagePart::Text { text: "a".into() }],
        };
        let mut messages = vec![text(); 127];
        messages.push(ImageMessage {
            role: Role::User,
            content: vec![image(ImageEncoding::Png, PNG)],
        });
        assert!(provider_messages(&OperationInput::ImageMessages {
            messages: messages.clone()
        })
        .is_ok());
        messages.push(text());
        assert_eq!(
            provider_messages(&OperationInput::ImageMessages { messages }),
            Err(ErrorCode::RequestLimit)
        );
        let messages = vec![
            ImageMessage {
                role: Role::System,
                content: vec![ImagePart::Text { text: "a".into() }; 64],
            },
            ImageMessage {
                role: Role::User,
                content: {
                    let mut content = vec![ImagePart::Text { text: "a".into() }; 64];
                    content.push(image(ImageEncoding::Png, PNG));
                    content
                },
            },
        ];
        assert_eq!(
            provider_messages(&OperationInput::ImageMessages { messages }),
            Err(ErrorCode::RequestLimit)
        );
    }
    #[test]
    fn empty_messages_parts_and_zero_dimensions_are_refused() {
        assert_eq!(
            provider_messages(&OperationInput::ImageMessages { messages: vec![] }),
            Err(ErrorCode::InvalidRequest)
        );
        assert_eq!(
            provider_messages(&OperationInput::ImageMessages {
                messages: vec![ImageMessage {
                    role: Role::User,
                    content: vec![]
                }]
            }),
            Err(ErrorCode::InvalidRequest)
        );
        assert_eq!(dimensions(0, 1), Err(ErrorCode::InvalidRequest));
        assert_eq!(dimensions(1, 0), Err(ErrorCode::InvalidRequest));
        assert_eq!(dimensions(4096, 1024), Ok(()));
    }
}
