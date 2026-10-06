//! Developer-local validation; reports metadata only, never proprietary payloads.
fn main() -> Result<(), String> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("usage: validate_gog GOG_APP_DIRECTORY")?;
    if root == "--profile" {
        println!("{}", civ3touch_core::assets::profile());
        return Ok(());
    }
    let assets = civ3touch_core::assets::load(std::path::Path::new(&root))?;
    println!("{}", assets.metadata());
    Ok(())
}
