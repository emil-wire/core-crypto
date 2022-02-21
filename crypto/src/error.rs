// Wire
// Copyright (C) 2022 Wire Swiss GmbH

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see http://www.gnu.org/licenses/.

/// CoreCrypto errors
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// This error is emitted when the requested conversation couldn't be found in our store
    #[error("Couldn't find conversation with id {0}")]
    ConversationNotFound(crate::ConversationId),
    /// This error is emitted when we find a malformed (i.e. not uuid) or empty identifier
    #[error("Malformed identifier found: {0}")]
    MalformedIdentifier(String),
    /// The keystore has no knowledge of such client; this shouldn't happen as Client::init is failsafe (find-else-create)
    #[error("The provided client signature has not been found in the keystore")]
    ClientSignatureNotFound,
    /// !!!! Something went very wrong and one of our locks has been poisoned by an in-thread panic !!!!
    #[error("One of the locks has been poisoned")]
    LockPoisonError,
    /// A conversation member is out of local stored keypackages - if it does happen something went wrong
    #[error("Member #{0} is out of keypackages")]
    OutOfKeyPackage(crate::member::MemberId),
    /// There was an issue when configuring a new conversation
    #[error(transparent)]
    ConversationConfigurationError(#[from] crate::conversation::MlsConversationConfigurationBuilderError),
    #[error(transparent)]
    CentralConfigurationError(#[from] crate::MlsCentralConfigurationBuilderError),
    /// Errors that are sent by our Keystore
    #[error(transparent)]
    KeyStoreError(#[from] core_crypto_keystore::CryptoKeystoreError),
    /// MLS Internal Errors
    #[error(transparent)]
    MlsError(#[from] MlsError),
    /// UUID-related errors
    #[error(transparent)]
    UuidError(#[from] uuid::Error),
    /// Error when parsing Strings that are not UTF-8
    #[error(transparent)]
    Utf8Error(#[from] std::str::Utf8Error),
    /// Error when trying to coerce ints into Strings
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    /// Other thingies
    #[error(transparent)]
    Other(#[from] eyre::Report),
}

pub type CryptoResult<T> = Result<T, CryptoError>;

/// MLS-specific error wrapper - see github.com/openmls/openmls for details
#[derive(Debug, thiserror::Error)]
pub enum MlsError {
    #[error(transparent)]
    MlsWelcomeError(#[from] openmls::prelude::WelcomeError),
    #[error(transparent)]
    MlsKeyPackageError(#[from] openmls::key_packages::KeyPackageError),
    #[error(transparent)]
    MlsConfigError(#[from] openmls::config::ConfigError),
    // #[error(transparent)]
    // MlsValidationError(#[from] openmls::prelude::ValidationError),
    // #[error(transparent)]
    // MlsVerificationError(#[from] openmls::prelude::VerificationError),
    #[error(transparent)]
    MlsInvalidMessageError(#[from] openmls::prelude::InvalidMessageError),
    #[error(transparent)]
    MlsEmptyInputError(#[from] openmls::prelude::EmptyInputError),
    #[error(transparent)]
    MlsCredentialError(#[from] openmls::prelude::CredentialError),
    // #[error(transparent)]
    // MlsMessageError(#[from] openmls::prelude::MlsMessageError),
    #[error(transparent)]
    MlsGroupError(#[from] openmls::prelude::MlsGroupError),
    #[error(transparent)]
    MlsCoreGroupError(#[from] openmls::prelude::CoreGroupError),
    // #[error(transparent)]
    // MlsCiphertextError(#[from] openmls::prelude::MlsCiphertextError),
    // #[error(transparent)]
    // MlsPlaintextError(#[from] openmls::prelude::MlsPlaintextError),
    #[error(transparent)]
    MlsErrorString(#[from] openmls::error::ErrorString),
}
