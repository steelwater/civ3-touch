use crate::error::FlicError;

#[derive(Debug)]
pub struct FlicAnimation {
    /// Width of the animation in pixels
    pub width: u16,
    /// Height of the animation in pixels
    pub height: u16,
    /// Number of frames in the animation
    pub frame_count: u16,
    /// Delay between frames in milliseconds
    pub frame_delay: u32,
    /// Number of direction sequences (e.g. 8 for N/NE/E/SE/S/SW/W/NW)
    pub num_animations: u16,
    /// Frames per direction (excluding the ring frame)
    pub animation_length: u16,
    /// RGBA data for all frames stored as one continuous buffer
    pub frame_data: Vec<u8>,
}

// FLIC file format constants
/// Magic number for FLC files (vs FLI)
const FLC_MAGIC: u16 = 0xAF12;

const CHUNK_COLOR_256: u16 = 4;
const CHUNK_DELTA_FLC: u16 = 7;
const CHUNK_COLOR_64: u16 = 11;
const CHUNK_BLACK: u16 = 13;
const CHUNK_BYTE_RUN: u16 = 15;
const CHUNK_LITERAL: u16 = 16;

/// FLIC header structure
#[allow(dead_code)]
#[repr(C, packed)]
#[derive(Debug)]
struct FlicHeader {
    /// total file size
    size: u32,
    /// magic number for version
    magic: u16,
    /// count of frames
    frame_count: u16,
    /// width in pixels
    width: u16,
    /// height in pixels
    height: u16,
    /// color depth, bits per pixel
    depth: u16,
    /// option flags
    flags: u16,
    /// delay between frames in milliseconds
    delay: u32,
    /// reserved space
    reserved1: u16,
    /// MS-DOS formatted date of creation
    created_date: u32,
    /// serial number of software that created this file
    creator: u32,
    /// MS-DOS formatted date of last update
    updated_date: u32,
    /// serial number of software that last updated this file
    updater: u32,
    /// x value of aspect ratio for display file was created on
    aspect_x: u16,
    /// y value of aspect ratio
    aspect_y: u16,
    /// extra reserved space
    reserved2: [u8; 38],
    /// offset from beginning of file to frame 1
    offset_frame1: u32,
    /// offset from beginning of file to frame 2
    offset_frame2: u32,
    /// size of the FlicAnimHeader struct (always 28 bytes); civ3 unique
    flic_anim_header_size: u32,
    /// FlicAnim flags; civ3 unique
    flic_anim_flags: u32,
    /// number of animations in this file; civ3 unique
    num_animations: u16,
    /// length of rames in each direction
    animation_length: u16,
    /// pixel offset from the left edge to the clipped sprite
    sprite_offset_left: u16,
    /// pixel offset from the top edge to the clipped sprite
    sprite_offset_top: u16,
    /// original unclipped sprite width
    sprite_width: u16,
    /// original unclipped sprite height
    sprite_height: u16,
    /// animation_length / 1000 * fps
    animation_time: u32,
    /// directions bitflag
    directions: u32,
    /// extra reserved space
    reserved3: [u8; 12],
}

impl FlicHeader {
    /// Parse FLIC header from bytes
    fn from_bytes(bytes: &[u8]) -> Result<Self, FlicError> {
        if bytes.len() < core::mem::size_of::<FlicHeader>() {
            return Err(FlicError::InvalidHeader);
        }

        let header = FlicHeader {
            size: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            magic: u16::from_le_bytes([bytes[4], bytes[5]]),
            frame_count: u16::from_le_bytes([bytes[6], bytes[7]]),
            width: u16::from_le_bytes([bytes[8], bytes[9]]),
            height: u16::from_le_bytes([bytes[10], bytes[11]]),
            depth: u16::from_le_bytes([bytes[12], bytes[13]]),
            flags: u16::from_le_bytes([bytes[14], bytes[15]]),
            delay: u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            reserved1: 0,
            created_date: u32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]),
            creator: u32::from_le_bytes([bytes[26], bytes[27], bytes[28], bytes[29]]),
            updated_date: u32::from_le_bytes([bytes[30], bytes[31], bytes[32], bytes[33]]),
            updater: u32::from_le_bytes([bytes[34], bytes[35], bytes[36], bytes[37]]),
            aspect_x: u16::from_le_bytes([bytes[38], bytes[39]]),
            aspect_y: u16::from_le_bytes([bytes[40], bytes[41]]),
            reserved2: [0; 38],
            offset_frame1: u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]),
            offset_frame2: u32::from_le_bytes([bytes[84], bytes[85], bytes[86], bytes[87]]),
            flic_anim_header_size: u32::from_le_bytes([bytes[88], bytes[89], bytes[90], bytes[91]]),
            flic_anim_flags: u32::from_le_bytes([bytes[92], bytes[93], bytes[94], bytes[95]]),
            num_animations: u16::from_le_bytes([bytes[96], bytes[97]]),
            animation_length: u16::from_le_bytes([bytes[98], bytes[99]]),
            sprite_offset_left: u16::from_le_bytes([bytes[100], bytes[101]]),
            sprite_offset_top: u16::from_le_bytes([bytes[102], bytes[103]]),
            sprite_width: u16::from_le_bytes([bytes[104], bytes[105]]),
            sprite_height: u16::from_le_bytes([bytes[106], bytes[107]]),
            animation_time: u32::from_le_bytes([bytes[108], bytes[109], bytes[110], bytes[111]]),
            directions: u32::from_le_bytes([bytes[112], bytes[113], bytes[114], bytes[115]]),
            reserved3: [0; 12],
        };

        Ok(header)
    }
}

#[repr(C, packed)]
#[derive(Debug)]
struct FrameHeader {
    size: u32,
    magic: u16,
    chunks: u16,
    reserved: [u8; 8],
}

impl FrameHeader {
    fn from_bytes(bytes: &[u8]) -> Result<Self, FlicError> {
        if bytes.len() < core::mem::size_of::<FrameHeader>() {
            return Err(FlicError::CorruptedChunk);
        }

        let header = FrameHeader {
            size: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            magic: u16::from_le_bytes([bytes[4], bytes[5]]),
            chunks: u16::from_le_bytes([bytes[6], bytes[7]]),
            reserved: [0; 8],
        };

        Ok(header)
    }
}

#[repr(C, packed)]
#[derive(Debug)]
struct ChunkHeader {
    size: u32,
    chunk_type: u16,
}

impl ChunkHeader {
    fn from_bytes(bytes: &[u8]) -> Result<Self, FlicError> {
        if bytes.len() < core::mem::size_of::<ChunkHeader>() {
            return Err(FlicError::CorruptedChunk);
        }

        let header = ChunkHeader {
            size: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            chunk_type: u16::from_le_bytes([bytes[4], bytes[5]]),
        };

        Ok(header)
    }
}

/// Read a FLIC animation from a byte buffer
pub fn read_flic(data: &[u8]) -> Result<FlicAnimation, FlicError> {
    // Parse header
    let header = FlicHeader::from_bytes(&data[0..])?;
    if header.magic != FLC_MAGIC {
        return Err(FlicError::UnsupportedType);
    }

    let width = header.width;
    let height = header.height;
    // each animation has a ring frame (same as the first frame), which isn't
    // counted in the frame count
    let frame_count = header.num_animations * (header.animation_length + 1);

    if width == 0 || height == 0 {
        return Err(FlicError::InvalidDimensions);
    }

    let bytes_per_frame = width as usize * height as usize * 4;
    let data_length = bytes_per_frame * frame_count as usize;

    let mut animation = FlicAnimation {
        width,
        height,
        frame_count,
        frame_delay: header.delay,
        num_animations: header.num_animations,
        animation_length: header.animation_length,
        frame_data: vec![0; data_length],
    };

    // Decode the animation data
    let mut current_frame: usize = 0;
    let mut read_offset = core::mem::size_of::<FlicHeader>();

    let mut palette: [u8; 1024] = [0xff; 1024];
    let mut indexed_frame: Vec<u8> = vec![0; width as usize * height as usize];

    while current_frame < animation.frame_count as usize {
        let frame_start = read_offset;
        let frame_header = FrameHeader::from_bytes(&data[read_offset..])?;
        read_offset += core::mem::size_of::<FrameHeader>();

        for _ in 0..frame_header.chunks {
            let chunk = ChunkHeader::from_bytes(&data[read_offset..])?;
            match chunk.chunk_type {
                CHUNK_COLOR_256 => {
                    let mut chunk_offset = core::mem::size_of::<ChunkHeader>();
                    let mut current_index = 0;

                    let packet_count = u16::from_le_bytes([
                        data[read_offset + chunk_offset],
                        data[read_offset + chunk_offset + 1],
                    ]);
                    chunk_offset += 2;

                    for _ in 0..packet_count {
                        let skip_count = data[read_offset + chunk_offset];
                        current_index += skip_count as usize * 4;
                        let mut change_count = data[read_offset + chunk_offset + 1] as usize;
                        if change_count == 0 {
                            change_count = 256;
                        }
                        chunk_offset += 2;
                        for _ in 0..change_count {
                            let r = data[read_offset + chunk_offset];
                            let g = data[read_offset + chunk_offset + 1];
                            let b = data[read_offset + chunk_offset + 2];
                            chunk_offset += 3;
                            palette[current_index] = r;
                            palette[current_index + 1] = g;
                            palette[current_index + 2] = b;
                            palette[current_index + 3] = 0xff;
                            current_index += 4;
                        }
                    }
                }
                CHUNK_DELTA_FLC => {
                    let mut chunk_offset = core::mem::size_of::<ChunkHeader>();
                    // number of rows to change
                    let chunk_rows = u16::from_le_bytes([
                        data[read_offset + chunk_offset],
                        data[read_offset + chunk_offset + 1],
                    ]);
                    chunk_offset += 2;

                    let mut current_row = 0;
                    for _ in 0..chunk_rows {
                        let mut skip_rows = 0;
                        let mut low_order_pad: Option<u8> = None;
                        while data[read_offset + chunk_offset] & 0x80 != 0
                            && low_order_pad.is_none()
                        {
                            if data[read_offset + chunk_offset] & 0x40 != 0 {
                                skip_rows += !u16::from_le_bytes([
                                    data[read_offset + chunk_offset],
                                    data[read_offset + chunk_offset + 1],
                                ]) + 1;
                            } else {
                                low_order_pad = Some(data[read_offset + chunk_offset + 1]);
                            }
                            chunk_offset += 2;
                        }

                        current_row += skip_rows;

                        let mut current_index = (current_row * width) as usize;

                        let packet_count = u16::from_le_bytes([
                            data[read_offset + chunk_offset],
                            data[read_offset + chunk_offset + 1],
                        ]);
                        chunk_offset += 2;

                        for _ in 0..packet_count {
                            let column_skip = data[read_offset + chunk_offset];
                            current_index += column_skip as usize;
                            let packet_type = data[read_offset + chunk_offset + 1];
                            chunk_offset += 2;

                            if packet_type & 0x80 == 0 {
                                // packet is a count of words to copy
                                for _ in 0..packet_type {
                                    indexed_frame[current_index] = data[read_offset + chunk_offset];
                                    indexed_frame[current_index + 1] =
                                        data[read_offset + chunk_offset + 1];
                                    chunk_offset += 2;
                                    current_index += 2;
                                }
                            } else {
                                // packet has a single word to be copied
                                let copies = !packet_type + 1;
                                for _ in 0..copies {
                                    indexed_frame[current_index] = data[read_offset + chunk_offset];
                                    indexed_frame[current_index + 1] =
                                        data[read_offset + chunk_offset + 1];
                                    current_index += 2;
                                }
                                chunk_offset += 2;
                            }
                        }
                        if let Some(pad) = low_order_pad {
                            if current_index & 1 == 0 {
                                println!("Even number of pixels, padding shouldn't be needed");
                            } else {
                                indexed_frame[current_index] = pad;
                            }
                        }
                        current_row += 1;
                    }
                }
                CHUNK_COLOR_64 => {
                    unimplemented!("64 COLOR PALETTE");
                }
                CHUNK_BLACK => {
                    unimplemented!("BLACK FRAME");
                }
                CHUNK_BYTE_RUN => {
                    let mut chunk_offset = 0;

                    chunk_offset += core::mem::size_of::<ChunkHeader>();
                    // skip the first byte, it's an unused count
                    chunk_offset += 1;
                    let mut pixel_index = 0;

                    while pixel_index < indexed_frame.len() {
                        let current_y = pixel_index / width as usize;

                        let packet_type: u8 = data[read_offset + chunk_offset];
                        if packet_type & 0x80 != 0 {
                            // copy literal pixels
                            let length = !packet_type + 1;
                            for _ in 0..length {
                                chunk_offset += 1;
                                indexed_frame[pixel_index] = data[read_offset + chunk_offset];
                                pixel_index += 1;
                            }
                            chunk_offset += 1;
                        } else {
                            // repeat a single pixel
                            let length = packet_type;
                            let pixel_value = data[read_offset + chunk_offset + 1];
                            for _ in 0..length {
                                indexed_frame[pixel_index] = pixel_value;
                                pixel_index += 1;
                            }
                            chunk_offset += 2;
                        }

                        if pixel_index / width as usize > current_y {
                            // skip a new packet-count
                            chunk_offset += 1;
                        }
                    }
                }
                CHUNK_LITERAL => {
                    unimplemented!("LITERAL FRAME");
                }
                _ => {
                    let ct = chunk.chunk_type;
                    unimplemented!("UNKNOWN CHUNK TYPE {}", ct);
                }
            }
            let chunk_size = if chunk.size == 0xcdcdcdcd {
                // known bug in FlicAnim for palettes, but these palettes
                // have known size
                // 6 bytes for header, 2 bytes for packet count, 2 for skip and
                // count, and then 256 3-byte colors
                6 + 2 + 2 + 256 * 3
            } else {
                chunk.size as usize
            };
            read_offset += chunk_size;
        }

        // Convert indexed frame to RGBA
        for (i, &pixel) in indexed_frame.iter().enumerate() {
            let palette_index = pixel as usize;
            let base = current_frame * bytes_per_frame + i * 4;
            if palette_index >= 254 {
                // Fully transparent (background)
                animation.frame_data[base] = 0;
                animation.frame_data[base + 1] = 0;
                animation.frame_data[base + 2] = 0;
                animation.frame_data[base + 3] = 0;
            } else if palette_index >= 248 {
                // Shadow values (248-253): semi-transparent black
                let shadow_level = 5 - (palette_index - 248);
                animation.frame_data[base] = 0;
                animation.frame_data[base + 1] = 0;
                animation.frame_data[base + 2] = 0;
                animation.frame_data[base + 3] = (shadow_level * 255 / 5) as u8;
            } else {
                animation.frame_data[base] = palette[palette_index * 4];
                animation.frame_data[base + 1] = palette[palette_index * 4 + 1];
                animation.frame_data[base + 2] = palette[palette_index * 4 + 2];
                animation.frame_data[base + 3] = 255;
            }
        }

        read_offset = frame_start + frame_header.size as usize;
        current_frame += 1;
    }

    Ok(animation)
}
