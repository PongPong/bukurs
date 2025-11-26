pub fn open_url(url: &str) -> crate::error::Result<()> {
    open::that(url)?;
    Ok(())
}
