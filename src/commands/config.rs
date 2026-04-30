use crate::config;
use anyhow::Result;
use owo_colors::OwoColorize;

pub fn show() -> Result<()> {
    let path = config::config_path()?;
    println!("{} {}", "Config file:".bold(), path.display());
    match config::load()? {
        Some(c) => {
            println!(
                "  {}: {}",
                "api_key".cyan(),
                config::mask_api_key(&c.api_key)
            );
            println!("  {}: {}", "location_provider".cyan(), c.location_provider);
            if let Some(name) = &c.default_place {
                println!("  {}: {name}", "default_place".cyan());
            }
            println!("  {}: {}", "language".cyan(), c.language);
            println!("  {}: {}", "region".cyan(), c.region);
        }
        None => println!("{}", "(Not configured. Run 'gmaps init'.)".yellow()),
    }
    Ok(())
}
