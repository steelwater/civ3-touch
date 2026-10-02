# flic

A Rust library for parsing FLIC (FLC/Autodesk Animator Pro) animation files, specifically targeting the extended format used by Civilization III.

## Overview

Civ3 ships its unit and effect animations as FLIC files with a proprietary header extension. This parser decodes those files into RGBA frame data suitable for rendering. It handles multiple animations per file, delta-compressed frames, and Civ3's shadow transparency system.

## Usage

```rust
let data = std::fs::read("warrior_attack.flc").unwrap();
let anim = flic::read_flic(&data).unwrap();

// anim.frame_data is a continuous RGBA8 buffer containing all frames
let frame_size = (anim.width as usize) * (anim.height as usize) * 4;
for i in 0..anim.frame_count as usize {
    let frame = &anim.frame_data[i * frame_size..(i + 1) * frame_size];
    // render frame...
}
```

### `FlicAnimation`

```rust
pub struct FlicAnimation {
    pub width: u16,
    pub height: u16,
    pub frame_count: u16,
    pub frame_delay: u32,       // milliseconds between frames
    pub frame_data: Vec<u8>,    // continuous RGBA8 buffer for all frames
}
```

### `FlicError`

```rust
pub enum FlicError {
    InvalidHeader,
    InvalidDimensions,
    UnsupportedType,
    UnsupportedFormat,
    CorruptedChunk,
    UnexpectedEndOfFile,
}
```

## File Format

### Header

The standard FLIC header is 128 bytes (little-endian), identified by magic number `0xAF12`. Civ3 extends this with an additional 28-byte animation header containing:

- Number of distinct animations in the file
- Frames per animation (excluding ring frame)
- Sprite offset and original dimensions
- Animation duration and direction bitflags

Total frames are calculated as `num_animations * (animation_length + 1)`, where the extra frame per animation is the "ring frame" (a duplicate of the first frame for looping).

### Frame Encoding

Each frame contains a sequence of typed chunks. The parser supports:

| Chunk Type | ID | Description |
|---|---|---|
| COLOR_256 | `0x04` | 256-color palette update (run-length encoded packets) |
| DELTA_FLC | `0x07` | Delta-encoded pixel changes from previous frame |
| BYTE_RUN | `0x0F` | Full-frame RLE-encoded pixel data |

Frames are decoded sequentially into a shared 256-color indexed buffer, then converted to RGBA8 on output.

### Palette and Transparency

The palette is stateful across frames — `COLOR_256` chunks update it in-place. Palette indices 248–255 are reserved for shadow/transparency:

| Index | Shadow Level | Alpha |
|---|---|---|
| 248 | 7 | 255 |
| 249 | 6 | 218 |
| 250 | 5 | 182 |
| 251 | 4 | 145 |
| 252 | 3 | 109 |
| 253 | 2 | 72 |
| 254 | 1 | 36 |
| 255 | 0 | 0 |

Shadow pixels are rendered as black with the corresponding alpha value.

### Delta Compression (DELTA_FLC)

The most complex chunk type. Encodes only the pixels that changed from the previous frame:

1. A line count header, followed by per-line opcodes
2. Opcodes encode row skips (via flag bits `0x80` and `0x40`), literal pixel runs, and repeat-pixel runs
3. Operates on 16-bit word-aligned data — pixels come in pairs
4. Supports scanline padding for odd-width images

### Civ3 Format Quirks

- **Chunk size sentinel:** A chunk size of `0xCDCDCDCD` signals a known Civ3 bug and is handled as a fixed-size chunk.
- **Extended header:** The extra 28 bytes after the standard header carry sprite metadata not present in standard FLIC files.
- **Ring frames:** Each animation includes an extra duplicate-of-first frame for seamless looping.

## CLI Tool: `fliccat`

Displays FLIC animations in a terminal that supports the Kitty graphics protocol.

```
cargo run --bin fliccat -- path/to/animation.flc
```

Frames are base64-encoded and transmitted as RGBA data using Kitty's `\e_G` escape sequences, respecting the animation's frame delay.

## Dependencies

None — pure Rust, standard library only.
