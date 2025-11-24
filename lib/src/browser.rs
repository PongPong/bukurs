use std::error::Error;

pub fn open_url(url: &str) -> Result<(), Box<dyn Error>> {
    open::that(url)?;
    Ok(())
}
