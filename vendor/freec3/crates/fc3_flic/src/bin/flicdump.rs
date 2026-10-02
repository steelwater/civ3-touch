//! Dump the palette from a FLIC file, highlighting transparency-related entries.

use std::env;
use std::fs;

const FLC_MAGIC: u16 = 0xAF12;
const CHUNK_COLOR_256: u16 = 4;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <flic_file>", args[0]);
        std::process::exit(1);
    }

    let data = fs::read(&args[1])?;
    if data.len() < 128 {
        return Err("File too small for FLIC header".into());
    }

    let magic = u16::from_le_bytes([data[4], data[5]]);
    if magic != FLC_MAGIC {
        return Err(format!("Not a FLC file (magic=0x{:04x})", magic).into());
    }

    let width = u16::from_le_bytes([data[8], data[9]]);
    let height = u16::from_le_bytes([data[10], data[11]]);
    println!("FLIC: {}x{}", width, height);

    // Parse frames looking for COLOR_256 palette chunks
    let header_size = 128;
    let mut palette = [0u8; 768]; // 256 * RGB
    let mut found_palette = false;

    let mut read_offset = header_size;
    // Just parse the first frame to get the palette
    if read_offset + 16 > data.len() {
        return Err("No frame data".into());
    }

    let _frame_size = u32::from_le_bytes([
        data[read_offset],
        data[read_offset + 1],
        data[read_offset + 2],
        data[read_offset + 3],
    ]) as usize;
    let chunk_count = u16::from_le_bytes([data[read_offset + 6], data[read_offset + 7]]);
    read_offset += 16; // frame header size

    for _ in 0..chunk_count {
        if read_offset + 6 > data.len() {
            break;
        }
        let chunk_size = u32::from_le_bytes([
            data[read_offset],
            data[read_offset + 1],
            data[read_offset + 2],
            data[read_offset + 3],
        ]);
        let chunk_type = u16::from_le_bytes([data[read_offset + 4], data[read_offset + 5]]);

        if chunk_type == CHUNK_COLOR_256 {
            // Parse the palette
            let mut off = read_offset + 6;
            let packet_count = u16::from_le_bytes([data[off], data[off + 1]]);
            off += 2;
            let mut pal_idx = 0usize;
            for _ in 0..packet_count {
                let skip = data[off] as usize;
                pal_idx += skip;
                let mut count = data[off + 1] as usize;
                if count == 0 {
                    count = 256;
                }
                off += 2;
                for _ in 0..count {
                    if pal_idx < 256 {
                        palette[pal_idx * 3] = data[off];
                        palette[pal_idx * 3 + 1] = data[off + 1];
                        palette[pal_idx * 3 + 2] = data[off + 2];
                    }
                    off += 3;
                    pal_idx += 1;
                }
            }
            found_palette = true;
        }

        let actual_size = if chunk_size == 0xcdcdcdcd {
            6 + 2 + 2 + 256 * 3
        } else {
            chunk_size as usize
        };
        read_offset += actual_size;
    }

    if !found_palette {
        println!("No COLOR_256 palette chunk found in first frame.");
        return Ok(());
    }

    // Dump full palette
    println!("\nFull palette (256 entries):");
    println!("{:>5}  {:>3} {:>3} {:>3}  Notes", "Index", "R", "G", "B");
    println!("{}", "-".repeat(40));

    let mut pure_green_indices = Vec::new();

    for i in 0..256 {
        let r = palette[i * 3];
        let g = palette[i * 3 + 1];
        let b = palette[i * 3 + 2];

        let mut notes = String::new();
        if r == 0 && g == 255 && b == 0 {
            notes.push_str(" <-- PURE GREEN (0,255,0)");
            pure_green_indices.push(i);
        }
        if r == 0 && g == 0 && b == 255 {
            notes.push_str(" <-- PURE BLUE (0,0,255)");
        }
        if r == 255 && g == 0 && b == 255 {
            notes.push_str(" <-- MAGENTA (255,0,255)");
        }
        if i >= 248 {
            notes.push_str(" [shadow/transparency range]");
        }

        // Only print entries that are interesting (non-zero, or in the special range)
        if r != 0 || g != 0 || b != 0 || i >= 248 || !notes.is_empty() {
            println!("  {:>3}  {:>3} {:>3} {:>3}{}", i, r, g, b, notes);
        }
    }

    println!("\n--- Key entries ---");
    for idx in [254, 255] {
        let r = palette[idx * 3];
        let g = palette[idx * 3 + 1];
        let b = palette[idx * 3 + 2];
        println!("  Index {}: ({}, {}, {})", idx, r, g, b);
    }

    if !pure_green_indices.is_empty() {
        println!(
            "\nPure green (0,255,0) found at indices: {:?}",
            pure_green_indices
        );
    } else {
        println!("\nNo pure green (0,255,0) found in palette.");
    }

    // Now use the real parser and scan ALL frames
    println!("\n--- Scanning all decoded RGBA frames ---");
    let animation = fc3_flic::read_flic(&data)?;
    let frame_pixels = animation.width as usize * animation.height as usize;
    let anim_len = animation.animation_length as usize + 1; // +1 for ring frame
    let num_anims = animation.num_animations as usize;
    let directions = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

    println!(
        "Animations: {}, frames per direction: {} (+1 ring = {})",
        num_anims, animation.animation_length, anim_len
    );
    println!("Total frames: {}", animation.frame_count);

    for dir in 0..num_anims {
        let dir_name = if dir < directions.len() {
            directions[dir]
        } else {
            "?"
        };
        let start_frame = dir * anim_len;
        let end_frame = start_frame + anim_len;

        let mut dir_green = 0;
        let mut dir_any_nonzero_alpha = 0;
        for frame in start_frame..end_frame.min(animation.frame_count as usize) {
            let offset = frame * frame_pixels * 4;
            let fdata = &animation.frame_data[offset..offset + frame_pixels * 4];

            for i in 0..frame_pixels {
                let r = fdata[i * 4];
                let g = fdata[i * 4 + 1];
                let b = fdata[i * 4 + 2];
                let a = fdata[i * 4 + 3];
                if a > 0 {
                    dir_any_nonzero_alpha += 1;
                }
                // Detect any visually green pixel (g channel dominant)
                if g > 100 && g > r.saturating_add(30) && g > b.saturating_add(30) && a > 0 {
                    dir_green += 1;
                    if dir_green <= 3 {
                        let x = i % animation.width as usize;
                        let y = i / animation.width as usize;
                        println!(
                            "  {} frame {} ({},{}): rgba({},{},{},{}) [palette region check]",
                            dir_name,
                            frame - start_frame,
                            x,
                            y,
                            r,
                            g,
                            b,
                            a
                        );
                    }
                }
            }
        }
        println!(
            "Dir {} ({}): {} green pixels, {} visible pixels",
            dir, dir_name, dir_green, dir_any_nonzero_alpha
        );
    }

    Ok(())
}
