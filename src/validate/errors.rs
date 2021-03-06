use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidateError {
    #[error("Missing or empty assets directory")]
    MissingOrEmptyAssetsDirectory,

    #[error("Invalid assets directory")]
    InvalidAssetsDirectory,

    #[error("Name exceeds 32 chars.")]
    NameTooLong,

    #[error("Symbol exceeds 10 chars.")]
    SymbolTooLong,

    #[error("Url exceeds 200 chars.")]
    UrlTooLong,

    #[error("Creator address: {0} is invalid.")]
    InvalidCreatorAddress(String),

    #[error("Creators' share does not equal 100%.")]
    InvalidCreatorShare,

    #[error("Seller fee basis points must be between 0 and 10,000.")]
    InvalidSellerFeeBasisPoints,

    #[error("Missing animation url field")]
    MissingAnimationUrl,

    #[error("Missing external url field")]
    MissingExternalUrl,

    #[error("Missing collection field")]
    MissingCollection,
}
