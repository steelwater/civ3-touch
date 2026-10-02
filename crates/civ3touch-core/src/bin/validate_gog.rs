//! Validate only the explicitly inventoried, developer-local prototype images.
use std::{fs, path::Path};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("usage: validate_gog GOG_APP_DIRECTORY")?;
    let root = Path::new(&root);
    for name in ["xggc", "xtgc", "xdgc", "xdgp", "xdpc", "xpgc"] {
        let relative = format!("Art/Terrain/{name}.pcx");
        let data = fs::read(root.join(&relative))?;
        let image = fc3_pcx::read_pcx(&data)?;
        if image.width == 0
            || image.height == 0
            || image.data.len() != image.width as usize * image.height as usize * 4
        {
            return Err(format!("invalid decoded dimensions: {relative}").into());
        }
        println!("PASS {relative}: {}x{} RGBA", image.width, image.height);
    }
    for name in ["settDefault", "settRun"] {
        let relative = format!("Art/Units/Settler/{name}.flc");
        let data = fs::read(root.join(&relative))?;
        let animation = fc3_flic::read_flic(&data)?;
        if animation.width == 0
            || animation.height == 0
            || animation.frame_data.is_empty()
            || animation.num_animations != 8
        {
            return Err(format!("invalid decoded animation: {relative}").into());
        }
        println!(
            "PASS {relative}: {}x{}, {} directions, {} frames/direction, {} RGBA bytes",
            animation.width,
            animation.height,
            animation.num_animations,
            animation.animation_length,
            animation.frame_data.len()
        );
    }
    Ok(())
}
