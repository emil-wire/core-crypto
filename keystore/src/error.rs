#[derive(Debug, thiserror::Error)]
pub enum MissingKeyErrorKind {
    #[error("MLS Key Bundle")]
    MlsKeyBundle,
    #[cfg(feature = "proteus_keystore")]
    #[error("Proteus PreKey")]
    ProteusPrekey,
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoKeystoreError {
    #[error("The requested {0} is not present in the store")]
    MissingKeyInStore(#[from] MissingKeyErrorKind),
    #[error("One of the locks has been poisoned")]
    LockPoisonError,
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    DbError(#[from] rusqlite::Error),
    #[error(transparent)]
    DbMigrationError(#[from] refinery::Error),
    #[cfg(test)]
    #[error(transparent)]
    KeyPackageError(#[from] openmls::prelude::KeyPackageError),
    #[cfg(feature = "proteus_keystore")]
    #[error(transparent)]
    PrekeyDecodeError(#[from] proteus::internal::types::DecodeError),
    #[cfg(feature = "proteus_keystore")]
    #[error(transparent)]
    PrekeyEncodeError(#[from] proteus::internal::types::EncodeError),
    #[error("{0}")]
    MlsKeyStoreError(String),
    #[error(transparent)]
    UuidError(#[from] uuid::Error),
    #[cfg(feature = "ios-wal-compat")]
    #[error(transparent)]
    HexSaltDecodeError(#[from] hex::FromHexError),
    #[error(transparent)]
    Other(#[from] eyre::Report),
}

pub type CryptoKeystoreResult<T> = Result<T, CryptoKeystoreError>;
