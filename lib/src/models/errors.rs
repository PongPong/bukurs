#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("A bookmark with URL '{0}' already exists")]
    DuplicateUrl(String),

    #[error("Database error")]
    DbError,
}
