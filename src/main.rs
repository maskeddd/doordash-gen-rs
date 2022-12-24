use anyhow::Result;
use doordash_gen::AccountGenerator;

fn main() -> Result<()> {
    AccountGenerator::new("config.toml", None)?.run()?;

    Ok(())
}
