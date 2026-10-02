# pcx

A Rust library for parsing PCX (ZSoft Paintbrush) image files, targeting the 256-color format used by Civilization III for its terrain, UI, and icon assets.

## Overview

Civ3 stores many of its static image assets as 8-bit indexed-color PCX files with a 256-color palette and RLE compression. This parser decodes them into RGBA data, handling Civ3's transparency convention where palette indices 254–255 are treated as fully transparent.

## Usage

```rust
let data = std::fs::read("grassland.pcx").unwrap();
let image = pcx::read_pcx(&data).unwrap();

// image.data is RGBA8: 4 bytes per pixel, row-major
let pixel_offset = (y * image.width as usize + x) * 4;
let r = image.data[pixel_offset];
let g = image.data[pixel_offset + 1];
let b = image.data[pixel_offset + 2];
let a = image.data[pixel_offset + 3];
```

### `PcxImage`

```rust
pub struct PcxImage {
    pub width: u16,
    pub height: u16,
    pub data: Vec<u8>,   // RGBA8 pixel data
}

impl PcxImage {
    pub fn as_slice(&self) -> &[u8];
    pub fn as_mut_slice(&mut self) -> &mut [u8];
}
```

### `PcxError`

```rust
pub enum PcxError {
    InvalidSignature,
    UnsupportedEncoding,
    InvalidHeader,
    InvalidDimensions,
    InvalidPalette,
    DecodingError,
}
```

## File Format

### Structure

A PCX file has three sections:

```
[ 128-byte header ] [ RLE-encoded scanlines ] [ 0x0C marker + 768-byte palette ]
```

The 256-color palette lives at the end of the file (last 768 bytes), preceded by a `0x0C` marker byte.

### Header

128 bytes, little-endian. Key fields:

| Offset | Size | Field | Expected |
|---|---|---|---|
| 0 | 1 | Signature | `0x0A` |
| 1 | 1 | Version | 5 (PCX 3.0) |
| 2 | 1 | Encoding | 1 (RLE) |
| 3 | 1 | Bits per pixel | 8 |
| 4–11 | 8 | Bounding box | x_min, y_min, x_max, y_max (u16) |
| 66 | 2 | Bytes per line | Scanline stride (may exceed width) |

Image dimensions: `width = x_max - x_min + 1`, `height = y_max - y_min + 1`.

### RLE Decoding

Each scanline is independently RLE-encoded:

- If a byte has its top two bits set (`byte & 0xC0 == 0xC0`), it's a **run**: the lower 6 bits are the repeat count (1–63), and the next byte is the pixel value.
- Otherwise, the byte is a **literal** pixel value.

Scanlines are padded to `bytes_per_line`; any excess beyond the image width is discarded.

### Palette and Transparency

The palette is 256 entries of 3 bytes each (RGB), stored at the end of the file. Palette indices 254 and 255 are treated as transparent — they produce `RGBA(0, 0, 0, 0)`. All other indices are fully opaque.

### Validation

The parser checks:
- Signature byte is `0x0A`
- Encoding byte is `1` (RLE)
- Dimensions are > 0 and ≤ 4096 in each axis
- Palette marker `0x0C` is present at the expected offset
- File is large enough to contain header + palette (minimum 128 + 769 bytes)

## CLI Tool: `pcxcat`

Displays PCX images in a terminal that supports the Kitty graphics protocol.

```
cargo run --bin pcxcat -- path/to/image.pcx
```

The image is base64-encoded and transmitted as RGBA data using Kitty's `\e_G` escape sequences.

## Dependencies

None — pure Rust, standard library only.
