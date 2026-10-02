//! A simple PCX image viewer for the terminal
//! Usage:
//!   pcxcat <pcx_file>
//! Running this in a terminal that supports the Kitty graphics protocol will
//! display the image.

use fc3_pcx::read_pcx;
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get filename from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <pcx_file>", args[0]);
        std::process::exit(1);
    }

    let filename = &args[1];

    // Read file into buffer
    let mut file = File::open(filename)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Parse PCX
    let image = read_pcx(&buffer)?;

    println!("PCX image dimensions: {}x{}", image.width, image.height);

    // Display the image using Kitty terminal graphics protocol
    display_image_kitty(&image.data, image.width as u32, image.height as u32)?;

    Ok(())
}

/// A simple base64 encoder; it didn't feel worth bringing a dependency for an
/// example/testing bin
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let padding_char = '=';

    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);

    // Process input in chunks of 3 bytes
    for chunk in data.chunks(3) {
        let b0 = chunk.first().copied().unwrap_or(0);
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        // Convert 3 bytes into 4 base64 characters
        let c0 = ALPHABET[(b0 >> 2) as usize];
        let c1 = ALPHABET[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize];
        let c2 = if chunk.len() > 1 {
            ALPHABET[(((b1 & 0x0F) << 2) | (b2 >> 6)) as usize]
        } else {
            b'='
        };
        let c3 = if chunk.len() > 2 {
            ALPHABET[(b2 & 0x3F) as usize]
        } else {
            b'='
        };

        result.push(c0 as char);
        result.push(c1 as char);
        result.push(if c2 == b'=' { padding_char } else { c2 as char });
        result.push(if c3 == b'=' { padding_char } else { c3 as char });
    }

    result
}

/// Display an RGBA image using the Kitty terminal graphics protocol
fn display_image_kitty(data: &[u8], width: u32, height: u32) -> io::Result<()> {
    // Encode the image data using our custom base64 encoder
    let encoded_data = base64_encode(data);

    // Split the data into chunks to avoid very long lines
    // Kitty protocol allows for chunked transfers
    const CHUNK_SIZE: usize = 2048; //4096;
    let mut stdout = io::stdout();

    // Format the kitty graphics command
    // Format: <ESC>_Gf=<format>,a=T,t=d,s=<width>,v=<height>;base64-data<ESC>\
    // - f=32 means RGBA format (32-bit)
    // - a=T means the action is transmit and display image
    // - t=d means we're sending the raw image data directly
    // - s=<width> and v=<height> specify the dimensions
    write!(stdout, "\x1b_Gf=32,a=T,t=d,s={},v={};", width, height)?;

    // Write data in chunks
    for chunk in encoded_data.as_bytes().chunks(CHUNK_SIZE) {
        stdout.write_all(chunk)?;
    }

    // Write the final chunk delimiter
    write!(stdout, "\x1b\\")?;
    stdout.flush()?;

    Ok(())
}
