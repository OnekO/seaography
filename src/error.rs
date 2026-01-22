use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum SeaographyError {
    #[error("[async_graphql] {0:?}")]
    AsyncGraphQLError(async_graphql::Error),
    #[error("[int conversion] {0}")]
    TryFromIntError(#[from] std::num::TryFromIntError),
    #[error("[parsing] {0}")]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error("[uuid] {0}")]
    UuidParseError(#[from] uuid::Error),
    #[error("[type conversion: {1}] {0}")]
    TypeConversionError(String, String),
    #[error("[array conversion] postgres array can not be nested type of array")]
    NestedArrayConversionError,
    #[error("[custom filter] {0}")]
    CustomFilterError(String),
    #[error("[async_graphql] {0:?}")]
    UploadError(async_graphql::InputValueError<async_graphql::Upload>),
    #[error("[database] {0}")]
    DbError(#[from] sea_orm::DbErr),
}

impl From<async_graphql::Error> for SeaographyError {
    fn from(error: async_graphql::Error) -> Self {
        SeaographyError::AsyncGraphQLError(error)
    }
}

impl From<async_graphql::InputValueError<async_graphql::Upload>> for SeaographyError {
    fn from(error: async_graphql::InputValueError<async_graphql::Upload>) -> Self {
        SeaographyError::UploadError(error)
    }
}

pub type SeaResult<T> = Result<T, SeaographyError>;
