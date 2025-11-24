#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("A bookmark with this URL already exists")]
    DuplicateUrl,

    #[error("Database error")]
    DbError,
}
