use anyhow::Result;
use doordash_gen::AccountGenerator;
use tracing::{error, info};

fn main() -> Result<()> {
    let mut generator = AccountGenerator::new("config.toml", None)?;
    generator.run()?;
    if generator.config.save_to_file.unwrap_or(true) {
        match generator.save_to_file(None) {
            Ok(path) => info!("Successfully saved accounts to {}", path),
            Err(err) => error!("Unable to save accounts to file: {}", err),
        }
    }
    Ok(())
}
