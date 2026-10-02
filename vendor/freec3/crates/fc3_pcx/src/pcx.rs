use crate::error::PcxError;

// PCX header is 128 bytes
const PCX_HEADER_SIZE: usize = 128;
const PCX_SIGNATURE: u8 = 0x0A;

/// Represents a PCX image in RGBA8 format
pub struct PcxImage {
    pub width: u16,
    pub height: u16,
    pub data: Vec<u8>, // RGBA8 data, 4 bytes per pixel
}

impl PcxImage {
    /// Creates a new empty PCX image with the specified dimensions
    pub fn new(width: u16, height: u16) -> Self {
        let size = width as usize * height as usize * 4;
        PcxImage {
            width,
            height,
            data: vec![0; size],
        }
    }

    /// Returns the image data as a slice
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    /// Returns the image data as a mutable slice
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

/// PCX header structure
#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug)]
struct PcxHeader {
    manufacturer: u8,
    version: u8,
    encoding: u8,
    bits_per_pixel: u8,
    x_min: u16,
    y_min: u16,
    x_max: u16,
    y_max: u16,
    h_dpi: u16,
    v_dpi: u16,
    palette: [u8; 48],
    reserved: u8,
    color_planes: u8,
    bytes_per_line: u16,
    palette_type: u16,
}

impl PcxHeader {
    /// Parse PCX header from bytes
    fn from_bytes(bytes: &[u8]) -> Result<Self, PcxError> {
        if bytes.len() < PCX_HEADER_SIZE {
            return Err(PcxError::InvalidHeader);
        }

        if bytes[0] != PCX_SIGNATURE {
            return Err(PcxError::InvalidSignature);
        }

        let header = PcxHeader {
            manufacturer: bytes[0],
            version: bytes[1],
            encoding: bytes[2],
            bits_per_pixel: bytes[3],
            x_min: u16::from_le_bytes([bytes[4], bytes[5]]),
            y_min: u16::from_le_bytes([bytes[6], bytes[7]]),
            x_max: u16::from_le_bytes([bytes[8], bytes[9]]),
            y_max: u16::from_le_bytes([bytes[10], bytes[11]]),
            h_dpi: u16::from_le_bytes([bytes[12], bytes[13]]),
            v_dpi: u16::from_le_bytes([bytes[14], bytes[15]]),
            palette: {
                let mut palette = [0u8; 48];
                palette.copy_from_slice(&bytes[16..64]);
                palette
            },
            reserved: bytes[64],
            color_planes: bytes[65],
            bytes_per_line: u16::from_le_bytes([bytes[66], bytes[67]]),
            palette_type: u16::from_le_bytes([bytes[68], bytes[69]]),
        };

        // Validate header
        if header.encoding != 1 {
            return Err(PcxError::UnsupportedEncoding);
        }

        Ok(header)
    }

    /// Get image width
    fn width(&self) -> u16 {
        self.x_max - self.x_min + 1
    }

    /// Get image height
    fn height(&self) -> u16 {
        self.y_max - self.y_min + 1
    }
}

/// Reads a PCX image from a byte buffer
pub fn read_pcx(data: &[u8]) -> Result<PcxImage, PcxError> {
    read_pcx_impl(data, |idx| idx >= 254)
}

/// Reads a PCX image, treating palette indices where `is_transparent(index)` returns true
/// as fully transparent pixels (alpha = 0).
pub fn read_pcx_with_transparency(
    data: &[u8],
    is_transparent: impl Fn(u8) -> bool,
) -> Result<PcxImage, PcxError> {
    read_pcx_impl(data, is_transparent)
}

fn read_pcx_impl(data: &[u8], is_transparent: impl Fn(u8) -> bool) -> Result<PcxImage, PcxError> {
    if data.len() < PCX_HEADER_SIZE {
        return Err(PcxError::InvalidHeader);
    }

    // Parse header
    let header = PcxHeader::from_bytes(&data[0..PCX_HEADER_SIZE])?;

    let width = header.width();
    let height = header.height();

    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(PcxError::InvalidDimensions);
    }

    // Check for 256-color palette marker at the end of the file
    if data.len() < PCX_HEADER_SIZE + 769 || data[data.len() - 769] != 0x0C {
        return Err(PcxError::InvalidPalette);
    }

    // Extract palette (256 entries, 3 bytes each for RGB)
    let palette_offset = data.len() - 768;
    let palette_data = &data[palette_offset..];

    // Create the output image
    let mut image = PcxImage::new(width, height);

    // Decode the image data
    decode_pcx_data(
        &data[PCX_HEADER_SIZE..palette_offset - 1],
        palette_data,
        &mut image,
        header.bytes_per_line,
        width,
        height,
        &is_transparent,
    )?;

    Ok(image)
}

/// Decode PCX RLE-encoded data to RGBA8
fn decode_pcx_data(
    encoded_data: &[u8],
    palette: &[u8],
    image: &mut PcxImage,
    bytes_per_line: u16,
    width: u16,
    height: u16,
    is_transparent: &dyn Fn(u8) -> bool,
) -> Result<(), PcxError> {
    let mut data_index = 0;
    let mut output_index = 0;

    // Decode each scanline
    for _ in 0..height {
        let mut line_index = 0;

        // Decode until we've read bytes_per_line or reached the end of the data
        while line_index < bytes_per_line && data_index < encoded_data.len() {
            let byte = encoded_data[data_index];
            data_index += 1;

            // Check if this is a run-length packet
            if (byte & 0xC0) == 0xC0 {
                // Extract run length (lower 6 bits)
                let run_length = byte & 0x3F;

                if data_index >= encoded_data.len() {
                    return Err(PcxError::DecodingError);
                }

                // Get the pixel value to repeat
                let pixel_value = encoded_data[data_index];
                data_index += 1;

                // Write the repeated pixel
                for _ in 0..run_length {
                    if line_index < width {
                        // Only write pixels within image width
                        let palette_index = pixel_value as usize;

                        if is_transparent(pixel_value) {
                            // Transparent pixel
                            image.data[output_index] = 0;
                            image.data[output_index + 1] = 0;
                            image.data[output_index + 2] = 0;
                            image.data[output_index + 3] = 0;
                        } else {
                            // Regular pixel
                            image.data[output_index] = palette[palette_index * 3];
                            image.data[output_index + 1] = palette[palette_index * 3 + 1];
                            image.data[output_index + 2] = palette[palette_index * 3 + 2];
                            image.data[output_index + 3] = 255; // Fully opaque
                        }

                        output_index += 4;
                    }
                    line_index += 1;
                }
            } else {
                // Single pixel
                if line_index < width {
                    // Only write pixels within image width
                    let palette_index = byte as usize;

                    if is_transparent(byte) {
                        // Transparent pixel
                        image.data[output_index] = 0;
                        image.data[output_index + 1] = 0;
                        image.data[output_index + 2] = 0;
                        image.data[output_index + 3] = 0;
                    } else {
                        // Regular pixel
                        image.data[output_index] = palette[palette_index * 3];
                        image.data[output_index + 1] = palette[palette_index * 3 + 1];
                        image.data[output_index + 2] = palette[palette_index * 3 + 2];
                        image.data[output_index + 3] = 255; // Fully opaque
                    }

                    output_index += 4;
                }
                line_index += 1;
            }
        }

        // Skip any remaining bytes in line (padding)
        while line_index < bytes_per_line && data_index < encoded_data.len() {
            let byte = encoded_data[data_index];
            data_index += 1;

            if (byte & 0xC0) == 0xC0 {
                // Skip RLE-encoded padding
                if data_index >= encoded_data.len() {
                    return Err(PcxError::DecodingError);
                }
                data_index += 1;
                line_index += (byte & 0x3F) as u16;
            } else {
                // Skip single padding byte
                line_index += 1;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcx_header_parsing() {
        // Create a fake PCX header for testing
        let mut header_data = [0u8; PCX_HEADER_SIZE];
        header_data[0] = PCX_SIGNATURE;
        header_data[1] = 5; // Version 3.0
        header_data[2] = 1; // RLE encoding
        header_data[3] = 8; // 8 bits per pixel

        // Set dimensions (32x32)
        header_data[4] = 0; // x_min low byte
        header_data[5] = 0; // x_min high byte
        header_data[6] = 0; // y_min low byte
        header_data[7] = 0; // y_min high byte
        header_data[8] = 31; // x_max low byte
        header_data[9] = 0; // x_max high byte
        header_data[10] = 31; // y_max low byte
        header_data[11] = 0; // y_max high byte

        // Set color planes to 1
        header_data[65] = 1;

        // Set bytes per line to 32
        header_data[66] = 32;
        header_data[67] = 0;

        let header = PcxHeader::from_bytes(&header_data).unwrap();
        assert_eq!(header.width(), 32);
        assert_eq!(header.height(), 32);
    }
}
