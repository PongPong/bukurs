use std::error::Error;

pub fn open_url(url: &str) -> Result<(), Box<dyn Error>> {
    open::that(url)?;
    Ok(())
}

pub fn auto_import() -> Result<(), Box<dyn Error>> {
    // Placeholder for auto-import logic
    // This would involve reading browser history/bookmarks databases
    // which is platform specific and complex.
    println!("Auto-import from browsers is not yet implemented.");
    Ok(())
}
