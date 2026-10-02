use fc3_flic::{read_flic, FlicAnimation};
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <flic_file>", args[0]);
        std::process::exit(1);
    }

    let filename = &args[1];

    // Read file into buffer
    let mut file = File::open(filename)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Parse FLIC
    let animation = read_flic(&buffer)?;

    println!(
        "FLIC animation dimensions: {}x{}",
        animation.width, animation.height
    );

    display_animation_kitty(&animation)?;

    Ok(())
}

fn display_animation_kitty(animation: &FlicAnimation) -> Result<(), io::Error> {
    // Display the animation using Kitty terminal graphics protocol
    let mut stdout = io::stdout();
    let mut frame = 0;

    loop {
        // Clear the screen
        stdout.write_all(b"\x1b[H\x1b[2J")?;

        // Display the current frame
        let image_data = &animation.frame_data[frame
            * (animation.width as usize * animation.height as usize * 4)
            ..(frame + 1) * (animation.width as usize * animation.height as usize * 4)];
        let encoded_data = base64_encode(image_data);

        display_image_kitty(
            &encoded_data,
            animation.width as u32,
            animation.height as u32,
        )?;

        // Wait for the next frame
        std::thread::sleep(std::time::Duration::from_millis(
            animation.frame_delay as u64,
        ));

        // Move to the next frame
        frame += 1;
        if frame >= animation.frame_count as usize {
            frame = 0;
        }
    }
}

fn display_image_kitty(data: &str, width: u32, height: u32) -> Result<(), io::Error> {
    // Display the image using Kitty terminal graphics protocol
    let mut stdout = io::stdout();

    // Set the image size
    write!(stdout, "\x1b_Gf=32,s={},v={},a=T,t=d;", width, height)?;

    const CHUNK_SIZE: usize = 4096;

    // Send the image data
    for chunk in data.as_bytes().chunks(CHUNK_SIZE) {
        stdout.write_all(chunk)?;
    }

    write!(stdout, "\x1b\\")?;
    stdout.flush()?;

    Ok(())
}

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
