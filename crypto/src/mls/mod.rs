use openmls::prelude::{Ciphersuite, KeyPackage, Welcome};
use openmls_traits::OpenMlsCryptoProvider;
use tls_codec::{Deserialize, Serialize};

use mls_crypto_provider::{MlsCryptoProvider, MlsCryptoProviderConfiguration};

use crate::prelude::{
    config::{MlsConversationConfiguration, MlsCustomConfiguration},
    identifier::ClientIdentifier,
    CiphersuiteName, Client, ClientId, ConversationId, CoreCryptoCallbacks, CryptoError, CryptoResult,
    MlsCentralConfiguration, MlsConversation, MlsCredentialType, MlsError,
};

pub(crate) mod client;
pub(crate) mod conversation;
pub(crate) mod credential;
pub(crate) mod external_commit;
pub(crate) mod external_proposal;
pub(crate) mod member;
pub(crate) mod proposal;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, derive_more::Deref, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
/// A wrapper for the OpenMLS Ciphersuite, so that we are able to provide a default value.
pub struct MlsCiphersuite(Ciphersuite);

impl MlsCiphersuite {
    /// Number of variants wrapped by this newtype
    /// We do it like this since we cannot apply `strum::EnumCount` on wrapped enum in openmls.
    /// It's fine since we'll probably have to redefine Ciphersuites here when we'll move to post-quantum
    pub(crate) const SIZE: usize = 7;
}

impl Default for MlsCiphersuite {
    fn default() -> Self {
        Self(Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519)
    }
}

impl From<Ciphersuite> for MlsCiphersuite {
    fn from(value: Ciphersuite) -> Self {
        Self(value)
    }
}

impl From<MlsCiphersuite> for Ciphersuite {
    fn from(ciphersuite: MlsCiphersuite) -> Self {
        ciphersuite.0
    }
}

impl From<MlsCiphersuite> for u16 {
    fn from(cs: MlsCiphersuite) -> Self {
        (&cs.0).into()
    }
}

impl TryFrom<u16> for MlsCiphersuite {
    type Error = CryptoError;

    fn try_from(c: u16) -> CryptoResult<Self> {
        Ok(CiphersuiteName::try_from(c)
            .map_err(|_| CryptoError::ImplementationError)?
            .into())
    }
}

// Prevents direct instantiation of [MlsCentralConfiguration]
pub(crate) mod config {
    use mls_crypto_provider::EntropySeed;

    use super::*;

    /// Configuration parameters for `MlsCentral`
    #[derive(Debug, Clone)]
    #[non_exhaustive]
    pub struct MlsCentralConfiguration {
        /// Location where the SQLite/IndexedDB database will be stored
        pub store_path: String,
        /// Identity key to be used to instantiate the [MlsCryptoProvider]
        pub identity_key: String,
        /// Identifier for the client to be used by [MlsCentral]
        pub client_id: Option<ClientId>,
        /// Entropy pool seed for the internal PRNG
        pub external_entropy: Option<EntropySeed>,
        /// All supported ciphersuites
        /// TODO: pending wire-server API supports selecting a ciphersuite only the first item of this array will be used.
        pub ciphersuites: Vec<MlsCiphersuite>,
    }

    impl MlsCentralConfiguration {
        /// Creates a new instance of the configuration.
        ///
        /// # Arguments
        /// * `store_path` - location where the SQLite/IndexedDB database will be stored
        /// * `identity_key` - identity key to be used to instantiate the [MlsCryptoProvider]
        /// * `client_id` - identifier for the client to be used by [MlsCentral]
        /// * `ciphersuites` - Ciphersuites supported by this device
        /// * `entropy` - External source of entropy for platforms where default source insufficient
        ///
        /// # Errors
        /// Any empty string parameter will result in a [CryptoError::MalformedIdentifier] error.
        ///
        /// # Examples
        ///
        /// This should fail:
        /// ```
        /// use core_crypto::{prelude::MlsCentralConfiguration, CryptoError};
        ///
        /// let result = MlsCentralConfiguration::try_new(String::new(), String::new(), Some(b"".to_vec().into()), vec![], None);
        /// assert!(matches!(result.unwrap_err(), CryptoError::MalformedIdentifier(_)));
        /// ```
        ///
        /// This should work:
        /// ```
        /// use core_crypto::{prelude::MlsCentralConfiguration, CryptoError};
        /// use core_crypto::mls::MlsCiphersuite;
        ///
        /// let result = MlsCentralConfiguration::try_new(
        ///     "/tmp/crypto".to_string(),
        ///     "MY_IDENTITY_KEY".to_string(),
        ///     Some(b"MY_CLIENT_ID".to_vec().into()),
        ///     vec![MlsCiphersuite::default()],
        ///     None,
        /// );
        /// assert!(result.is_ok());
        /// ```
        pub fn try_new(
            store_path: String,
            identity_key: String,
            client_id: Option<ClientId>,
            ciphersuites: Vec<MlsCiphersuite>,
            entropy: Option<Vec<u8>>,
        ) -> CryptoResult<Self> {
            // TODO: probably more complex rules to enforce
            if store_path.trim().is_empty() {
                return Err(CryptoError::MalformedIdentifier("store_path"));
            }
            // TODO: probably more complex rules to enforce
            if identity_key.trim().is_empty() {
                return Err(CryptoError::MalformedIdentifier("identity_key"));
            }
            // TODO: probably more complex rules to enforce
            if let Some(client_id) = client_id.as_ref() {
                if client_id.is_empty() {
                    return Err(CryptoError::MalformedIdentifier("client_id"));
                }
            }
            let external_entropy = entropy
                .as_deref()
                .map(|seed| &seed[..EntropySeed::EXPECTED_LEN])
                .map(EntropySeed::try_from_slice)
                .transpose()?;
            Ok(Self {
                store_path,
                identity_key,
                client_id,
                ciphersuites,
                external_entropy,
            })
        }

        /// Sets the entropy seed
        pub fn set_entropy(&mut self, entropy: EntropySeed) {
            self.external_entropy = Some(entropy);
        }

        #[cfg(test)]
        #[allow(dead_code)]
        /// Creates temporary file to prevent test collisions which would happen with hardcoded file path
        /// Intended to be used only in tests.
        pub(crate) fn tmp_store_path(tmp_dir: &tempfile::TempDir) -> String {
            let path = tmp_dir.path().join("store.edb");
            std::fs::File::create(&path).unwrap();
            path.to_str().unwrap().to_string()
        }
    }
}

/// The entry point for the MLS CoreCrypto library. This struct provides all functionality to create
/// and manage groups, make proposals and commits.
#[derive(Debug)]
pub struct MlsCentral {
    pub(crate) mls_client: Option<Client>,
    pub(crate) mls_backend: MlsCryptoProvider,
    pub(crate) mls_groups: crate::group_store::GroupStore<MlsConversation>,
    // pub(crate) mls_groups: HashMap<ConversationId, MlsConversation>,
    pub(crate) callbacks: Option<Box<dyn CoreCryptoCallbacks + 'static>>,
}

impl MlsCentral {
    /// Tries to initialize the MLS Central object.
    /// Takes a store path (i.e. Disk location of the embedded database, should be consistent between messaging sessions)
    /// And a root identity key (i.e. enclaved encryption key for this device)
    ///
    /// # Arguments
    /// * `configuration` - the configuration for the `MlsCentral`
    ///
    /// # Errors
    /// Failures in the initialization of the KeyStore can cause errors, such as IO, the same kind
    /// of errors can happen when the groups are being restored from the KeyStore or even during
    /// the client initialization (to fetch the identity signature). Other than that, `MlsError`
    /// can be caused by group deserialization or during the initialization of the credentials:
    /// * for x509 Credentials if the cetificate chain length is lower than 2
    /// * for Basic Credentials if the signature key cannot be generated either by not supported
    /// scheme or the key generation fails
    pub async fn try_new(configuration: MlsCentralConfiguration) -> CryptoResult<Self> {
        // Init backend (crypto + rand + keystore)
        let mls_backend = MlsCryptoProvider::try_new_with_configuration(MlsCryptoProviderConfiguration {
            db_path: &configuration.store_path,
            identity_key: &configuration.identity_key,
            in_memory: false,
            entropy_seed: configuration.external_entropy,
        })
        .await?;
        let mls_client = if let Some(id) = configuration.client_id {
            // Init client identity (load or create)
            Some(
                Client::init(
                    ClientIdentifier::Basic(id),
                    configuration.ciphersuites.as_slice(),
                    &mls_backend,
                )
                .await?,
            )
        } else {
            None
        };

        // Restore persisted groups if there are any
        let mls_groups = Self::restore_groups(&mls_backend).await?;

        Ok(Self {
            mls_backend,
            mls_client,
            mls_groups,
            callbacks: None,
        })
    }

    /// Same as the [crate::MlsCentral::try_new] but instead, it uses an in memory KeyStore. Although required, the `store_path` parameter from the `MlsCentralConfiguration` won't be used here.
    pub async fn try_new_in_memory(configuration: MlsCentralConfiguration) -> CryptoResult<Self> {
        let mls_backend = MlsCryptoProvider::try_new_with_configuration(MlsCryptoProviderConfiguration {
            db_path: &configuration.store_path,
            identity_key: &configuration.identity_key,
            in_memory: true,
            entropy_seed: configuration.external_entropy,
        })
        .await?;
        let mls_client = if let Some(id) = configuration.client_id {
            Some(
                Client::init(
                    ClientIdentifier::Basic(id),
                    configuration.ciphersuites.as_slice(),
                    &mls_backend,
                )
                .await?,
            )
        } else {
            None
        };
        let mls_groups = Self::restore_groups(&mls_backend).await?;

        Ok(Self {
            mls_backend,
            mls_client,
            mls_groups,
            callbacks: None,
        })
    }

    /// Initializes the MLS client if [CoreCrypto] has previously been initialized with
    /// [CoreCrypto::deferred_init] instead of [CoreCrypto::new].
    /// This should stay as long as proteus is supported. Then it should be removed.
    pub async fn mls_init(
        &mut self,
        identifier: ClientIdentifier,
        ciphersuites: Vec<MlsCiphersuite>,
    ) -> CryptoResult<()> {
        if self.mls_client.is_some() {
            // prevents wrong usage of the method instead of silently hiding the mistake
            return Err(CryptoError::ImplementationError);
        }
        let mls_client = Client::init(identifier, &ciphersuites, &self.mls_backend).await?;
        self.mls_client = Some(mls_client);
        Ok(())
    }

    /// Generates MLS KeyPairs/CredentialBundle with a temporary, random client ID.
    /// This method is designed to be used in conjunction with [MlsCentral::mls_init_with_client_id] and represents the first step in this process.
    ///
    /// This returns the TLS-serialized identity keys (i.e. the signature keypair's public key)
    pub async fn mls_generate_keypairs(&self, ciphersuites: Vec<MlsCiphersuite>) -> CryptoResult<Vec<Vec<u8>>> {
        if self.mls_client.is_some() {
            // prevents wrong usage of the method instead of silently hiding the mistake
            return Err(CryptoError::ImplementationError);
        }

        Client::generate_raw_keypairs(&ciphersuites, &self.mls_backend).await
    }

    /// Updates the current temporary Client ID with the newly provided one. This is the second step in the externally-generated clients process
    ///
    /// Important: This is designed to be called after [MlsCentral::mls_generate_keypairs]
    pub async fn mls_init_with_client_id(
        &mut self,
        client_id: ClientId,
        signature_public_keys: Vec<Vec<u8>>,
        ciphersuites: Vec<MlsCiphersuite>,
    ) -> CryptoResult<()> {
        if self.mls_client.is_some() {
            // prevents wrong usage of the method instead of silently hiding the mistake
            return Err(CryptoError::ImplementationError);
        }

        let mls_client =
            Client::init_with_external_client_id(client_id, signature_public_keys, &ciphersuites, &self.mls_backend)
                .await?;

        self.mls_client = Some(mls_client);
        Ok(())
    }

    /// Restore existing groups from the KeyStore.
    async fn restore_groups(
        backend: &MlsCryptoProvider,
    ) -> CryptoResult<crate::group_store::GroupStore<MlsConversation>> {
        use core_crypto_keystore::CryptoKeystoreMls as _;
        let groups = backend.key_store().mls_groups_restore().await?;

        let mut group_store = crate::group_store::GroupStore::default();

        if groups.is_empty() {
            return Ok(group_store);
        }

        for (group_id, (parent_id, state)) in groups.into_iter() {
            let conversation = MlsConversation::from_serialized_state(state, parent_id)?;
            if group_store.try_insert(group_id, conversation).is_err() {
                break;
            }
        }

        Ok(group_store)
    }

    /// [MlsCentral] is supposed to be a singleton. Knowing that, it does some optimizations by
    /// keeping MLS groups in memory. Sometimes, especially on iOS, it is required to use extensions
    /// to perform tasks in the background. Extensions are executed in another process so another
    /// [MlsCentral] instance has to be used. This method has to be used to synchronize instances.
    /// It simply fetches the MLS group from keystore in memory.
    pub async fn restore_from_disk(&mut self) -> CryptoResult<()> {
        self.mls_groups = Self::restore_groups(&self.mls_backend).await?;
        Ok(())
    }

    /// Sets the consumer callbacks (i.e authorization callbacks for CoreCrypto to perform authorization calls when needed)
    ///
    /// # Arguments
    /// * `callbacks` - a callback to be called to perform authorization
    pub fn callbacks(&mut self, callbacks: Box<dyn CoreCryptoCallbacks>) {
        self.callbacks = Some(callbacks);
    }

    /// Returns the client's public signature key as a buffer.
    /// Used to upload a public key to the server in order to verify client's messages signature.
    ///
    /// NB: only works for [MlsCredentialType::Basic]. For [MlsCredentialType::X509] we have trust anchor
    /// certificates provided by the backend hence no client registration is required.
    ///
    /// # Arguments
    /// * `ciphersuite` - a callback to be called to perform authorization
    pub fn client_public_key(&self, ciphersuite: MlsCiphersuite) -> CryptoResult<Vec<u8>> {
        let mls_client = self.mls_client.as_ref().ok_or(CryptoError::MlsNotInitialized)?;
        let cb = mls_client.find_credential_bundle(ciphersuite, MlsCredentialType::Basic)?;
        Ok(cb.credential().signature_key().as_slice().to_vec())
    }

    /// Returns the client's id as a buffer
    pub fn client_id(&self) -> CryptoResult<ClientId> {
        Ok(self
            .mls_client
            .as_ref()
            .ok_or(CryptoError::MlsNotInitialized)?
            .id()
            .clone())
    }

    /// Returns `amount_requested` OpenMLS [`KeyPackageBundle`]s.
    /// Will always return the requested amount as it will generate the necessary (lacking) amount on-the-fly
    ///
    /// Note: Keypackage pruning is performed as a first step
    ///
    /// # Arguments
    /// * `amount_requested` - number of KeyPackages to request and fill the `KeyPackageBundle`
    ///
    /// # Return type
    /// A vector of `KeyPackageBundle`
    ///
    /// # Errors
    /// Errors can happen when accessing the KeyStore
    pub async fn get_or_create_client_keypackages(
        &self,
        ciphersuite: MlsCiphersuite,
        amount_requested: usize,
    ) -> CryptoResult<Vec<KeyPackage>> {
        let mls_client = self.mls_client.as_ref().ok_or(CryptoError::MlsNotInitialized)?;
        mls_client
            .request_key_packages(amount_requested, ciphersuite, &self.mls_backend)
            .await
    }

    /// Returns the count of valid, non-expired, unclaimed keypackages in store for the given [MlsCiphersuite]
    pub async fn client_valid_key_packages_count(&self, ciphersuite: MlsCiphersuite) -> CryptoResult<usize> {
        self.mls_client
            .as_ref()
            .ok_or(CryptoError::MlsNotInitialized)?
            .valid_keypackages_count(&self.mls_backend, ciphersuite)
            .await
    }

    /// Create a new empty conversation
    ///
    /// # Arguments
    /// * `id` - identifier of the group/conversation (must be unique otherwise the existing group
    /// will be overridden)
    /// * `creator_credential_type` - kind of credential the creator wants to create the group with
    /// * `config` - configuration of the group/conversation
    ///
    /// # Errors
    /// Errors can happen from the KeyStore or from OpenMls for ex if no [KeyPackageBundle] can
    /// be found in the KeyStore
    pub async fn new_conversation(
        &mut self,
        id: ConversationId,
        creator_credential_type: MlsCredentialType,
        config: MlsConversationConfiguration,
    ) -> CryptoResult<()> {
        let mls_client = self.mls_client.as_mut().ok_or(CryptoError::MlsNotInitialized)?;
        let conversation = MlsConversation::create(
            id.clone(),
            mls_client,
            creator_credential_type,
            config,
            &self.mls_backend,
        )
        .await?;

        self.mls_groups.insert(id, conversation);

        Ok(())
    }

    /// Checks if a given conversation id exists locally
    pub async fn conversation_exists(&mut self, id: &ConversationId) -> bool {
        self.mls_groups
            .get_fetch(id, self.mls_backend.borrow_keystore_mut(), None)
            .await
            .ok()
            .flatten()
            .is_some()
    }

    /// Returns the epoch of a given conversation
    ///
    /// # Errors
    /// If the conversation can't be found
    pub async fn conversation_epoch(&mut self, id: &ConversationId) -> CryptoResult<u64> {
        Ok(self
            .mls_groups
            .get_fetch(id, self.mls_backend.borrow_keystore_mut(), None)
            .await?
            .ok_or_else(|| CryptoError::ConversationNotFound(id.to_owned()))?
            .read()
            .await
            .group
            .epoch()
            .as_u64())
    }

    /// Create a conversation from a received MLS Welcome message
    ///
    /// # Arguments
    /// * `welcome` - a `Welcome` message received as a result of a commit adding new members to a group
    /// * `configuration` - configuration of the group/conversation
    ///
    /// # Return type
    /// This function will return the conversation/group id
    ///
    /// # Errors
    /// Errors can be originating from the KeyStore of from OpenMls:
    /// * if no [KeyPackageBundle] can be read from the KeyStore
    /// * if the message can't be decrypted
    pub async fn process_welcome_message(
        &mut self,
        welcome: Welcome,
        custom_cfg: MlsCustomConfiguration,
    ) -> CryptoResult<ConversationId> {
        let configuration = MlsConversationConfiguration {
            custom: custom_cfg,
            ..Default::default()
        };
        let conversation = MlsConversation::from_welcome_message(welcome, configuration, &self.mls_backend).await?;
        let conversation_id = conversation.id.clone();
        self.mls_groups.insert(conversation_id.clone(), conversation);

        Ok(conversation_id)
    }

    /// Create a conversation from a TLS serialized MLS Welcome message. The `MlsConversationConfiguration` used in this function will be the default implementation.
    ///
    /// # Arguments
    /// * `welcome` - a TLS serialized welcome message
    /// * `configuration` - configuration of the MLS conversation fetched from the Delivery Service
    ///
    /// # Return type
    /// This function will return the conversation/group id
    ///
    /// # Errors
    /// see [MlsCentral::process_welcome_message]
    pub async fn process_raw_welcome_message(
        &mut self,
        welcome: Vec<u8>,
        custom_cfg: MlsCustomConfiguration,
    ) -> CryptoResult<ConversationId> {
        let mut cursor = std::io::Cursor::new(welcome);
        let welcome = Welcome::tls_deserialize(&mut cursor).map_err(MlsError::from)?;
        self.process_welcome_message(welcome, custom_cfg).await
    }

    /// Exports a TLS-serialized view of the current group state corresponding to the provided conversation ID.
    ///
    /// # Arguments
    /// * `conversation` - the group/conversation id
    /// * `message` - the encrypted message as a byte array
    ///
    /// # Return type
    /// A TLS serialized byte array of the `PublicGroupState`
    ///
    /// # Errors
    /// If the conversation can't be found, an error will be returned. Other errors are originating
    /// from OpenMls and serialization
    pub async fn export_public_group_state(&mut self, conversation_id: &ConversationId) -> CryptoResult<Vec<u8>> {
        let conversation = self.get_conversation(conversation_id).await?;
        let state = conversation
            .read()
            .await
            .group
            .export_public_group_state(&self.mls_backend)
            .await
            .map_err(MlsError::from)?;

        Ok(state.tls_serialize_detached().map_err(MlsError::from)?)
    }

    /// Closes the connection with the local KeyStore
    ///
    /// # Errors
    /// KeyStore errors, such as IO
    pub async fn close(self) -> CryptoResult<()> {
        self.mls_backend.close().await?;
        Ok(())
    }

    /// Destroys everything we have, in-memory and on disk.
    ///
    /// # Errors
    /// KeyStore errors, such as IO
    pub async fn wipe(self) -> CryptoResult<()> {
        self.mls_backend.destroy_and_reset().await?;
        Ok(())
    }

    /// Generates a random byte array of the specified size
    pub fn random_bytes(&self, len: usize) -> CryptoResult<Vec<u8>> {
        use openmls_traits::random::OpenMlsRand as _;
        Ok(self.mls_backend.rand().random_vec(len)?)
    }

    /// Returns a reference for the internal Crypto Provider
    pub fn provider(&self) -> &MlsCryptoProvider {
        &self.mls_backend
    }

    /// Returns a mutable reference for the internal Crypto Provider
    pub fn provider_mut(&mut self) -> &mut MlsCryptoProvider {
        &mut self.mls_backend
    }
}

#[cfg(test)]
pub mod tests {
    use wasm_bindgen_test::*;

    use crate::prelude::{CertificateBundle, ClientIdentifier, MlsCredentialType};
    use crate::{
        mls::{CryptoError, MlsCentral, MlsCentralConfiguration},
        test_utils::*,
    };

    wasm_bindgen_test_configure!(run_in_browser);

    pub mod conversation_epoch {
        use super::*;

        #[apply(all_cred_cipher)]
        #[wasm_bindgen_test]
        pub async fn can_get_newly_created_conversation_epoch(case: TestCase) {
            run_test_with_central(case.clone(), move |[mut central]| {
                Box::pin(async move {
                    let id = conversation_id();
                    central
                        .new_conversation(id.clone(), case.credential_type, case.cfg.clone())
                        .await
                        .unwrap();
                    let epoch = central.conversation_epoch(&id).await.unwrap();
                    assert_eq!(epoch, 0);
                })
            })
            .await;
        }

        #[apply(all_cred_cipher)]
        #[wasm_bindgen_test]
        pub async fn can_get_conversation_epoch(case: TestCase) {
            run_test_with_client_ids(
                case.clone(),
                ["alice", "bob"],
                move |[mut alice_central, mut bob_central]| {
                    Box::pin(async move {
                        let id = conversation_id();
                        alice_central
                            .new_conversation(id.clone(), case.credential_type, case.cfg.clone())
                            .await
                            .unwrap();
                        alice_central
                            .invite(&id, &mut bob_central, case.custom_cfg())
                            .await
                            .unwrap();
                        let epoch = alice_central.conversation_epoch(&id).await.unwrap();
                        assert_eq!(epoch, 1);
                    })
                },
            )
            .await;
        }

        #[apply(all_cred_cipher)]
        #[wasm_bindgen_test]
        pub async fn conversation_not_found(case: TestCase) {
            run_test_with_central(case.clone(), move |[mut central]| {
                Box::pin(async move {
                    let id = conversation_id();
                    let err = central.conversation_epoch(&id).await.unwrap_err();
                    assert!(matches!(err, CryptoError::ConversationNotFound(conv_id) if conv_id == id));
                })
            })
            .await;
        }
    }

    pub mod invariants {
        use crate::prelude::MlsCiphersuite;

        use super::*;

        #[apply(all_cred_cipher)]
        #[wasm_bindgen_test]
        pub async fn can_create_from_valid_configuration(case: TestCase) {
            run_tests(move |[tmp_dir_argument]| {
                Box::pin(async move {
                    let configuration = MlsCentralConfiguration::try_new(
                        tmp_dir_argument,
                        "test".to_string(),
                        Some("alice".into()),
                        vec![case.ciphersuite()],
                        None,
                    )
                    .unwrap();

                    let central = MlsCentral::try_new(configuration).await;
                    assert!(central.is_ok())
                })
            })
            .await
        }

        #[test]
        #[wasm_bindgen_test]
        pub fn store_path_should_not_be_empty_nor_blank() {
            let ciphersuites = vec![MlsCiphersuite::default()];
            let configuration = MlsCentralConfiguration::try_new(
                " ".to_string(),
                "test".to_string(),
                Some("alice".into()),
                ciphersuites,
                None,
            );
            assert!(matches!(
                configuration.unwrap_err(),
                CryptoError::MalformedIdentifier("store_path")
            ));
        }

        #[cfg_attr(not(target_family = "wasm"), async_std::test)]
        #[wasm_bindgen_test]
        pub async fn identity_key_should_not_be_empty_nor_blank() {
            run_tests(|[tmp_dir_argument]| {
                Box::pin(async move {
                    let ciphersuites = vec![MlsCiphersuite::default()];
                    let configuration = MlsCentralConfiguration::try_new(
                        tmp_dir_argument,
                        " ".to_string(),
                        Some("alice".into()),
                        ciphersuites,
                        None,
                    );
                    assert!(matches!(
                        configuration.unwrap_err(),
                        CryptoError::MalformedIdentifier("identity_key")
                    ));
                })
            })
            .await
        }

        #[cfg_attr(not(target_family = "wasm"), async_std::test)]
        #[wasm_bindgen_test]
        pub async fn client_id_should_not_be_empty() {
            run_tests(|[tmp_dir_argument]| {
                Box::pin(async move {
                    let ciphersuites = vec![MlsCiphersuite::default()];
                    let configuration = MlsCentralConfiguration::try_new(
                        tmp_dir_argument,
                        "test".to_string(),
                        Some("".into()),
                        ciphersuites,
                        None,
                    );
                    assert!(matches!(
                        configuration.unwrap_err(),
                        CryptoError::MalformedIdentifier("client_id")
                    ));
                })
            })
            .await
        }
    }

    pub mod persistence {
        use super::*;
        use std::collections::HashMap;

        #[apply(all_cred_cipher)]
        #[wasm_bindgen_test]
        pub async fn can_persist_group_state(case: TestCase) {
            run_tests(move |[store_path]| {
                Box::pin(async move {
                    let cid = match case.credential_type {
                        MlsCredentialType::Basic => ClientIdentifier::Basic("potato".into()),
                        MlsCredentialType::X509 => {
                            let cert = CertificateBundle::rand(case.cfg.ciphersuite, "potato".into());
                            ClientIdentifier::X509(HashMap::from([(case.cfg.ciphersuite, cert)]))
                        }
                    };
                    let configuration = MlsCentralConfiguration::try_new(
                        store_path,
                        "test".to_string(),
                        None,
                        vec![case.ciphersuite()],
                        None,
                    )
                    .unwrap();

                    let mut central = MlsCentral::try_new(configuration.clone()).await.unwrap();
                    central.mls_init(cid, vec![case.ciphersuite()]).await.unwrap();
                    let id = conversation_id();
                    let _ = central
                        .new_conversation(id.clone(), case.credential_type, case.cfg.clone())
                        .await;

                    central.close().await.unwrap();
                    let mut central = MlsCentral::try_new(configuration).await.unwrap();
                    let _ = central.encrypt_message(&id, b"Test").await.unwrap();

                    central.mls_backend.destroy_and_reset().await.unwrap();
                })
            })
            .await
        }

        #[apply(all_cred_cipher)]
        #[wasm_bindgen_test]
        pub async fn can_restore_group_from_db(case: TestCase) {
            run_tests(move |[alice_path, bob_path]| {
                Box::pin(async move {
                    let id = conversation_id();

                    let (alice_cid, bob_cid) = match case.credential_type {
                        MlsCredentialType::Basic => (
                            ClientIdentifier::Basic("alice".into()),
                            ClientIdentifier::Basic("bob".into()),
                        ),
                        MlsCredentialType::X509 => {
                            let cert = CertificateBundle::rand(case.cfg.ciphersuite, "alice".into());
                            let alice = ClientIdentifier::X509(HashMap::from([(case.cfg.ciphersuite, cert)]));
                            let cert = CertificateBundle::rand(case.cfg.ciphersuite, "bob".into());
                            let bob = ClientIdentifier::X509(HashMap::from([(case.cfg.ciphersuite, cert)]));
                            (alice, bob)
                        }
                    };
                    let alice_cfg = MlsCentralConfiguration::try_new(
                        alice_path,
                        "test".to_string(),
                        None,
                        vec![case.ciphersuite()],
                        None,
                    )
                    .unwrap();
                    let mut alice_central = MlsCentral::try_new(alice_cfg.clone()).await.unwrap();
                    alice_central
                        .mls_init(alice_cid, vec![case.ciphersuite()])
                        .await
                        .unwrap();

                    let bob_cfg = MlsCentralConfiguration::try_new(
                        bob_path,
                        "test".to_string(),
                        None,
                        vec![case.ciphersuite()],
                        None,
                    )
                    .unwrap();
                    let mut bob_central = MlsCentral::try_new(bob_cfg).await.unwrap();
                    bob_central.mls_init(bob_cid, vec![case.ciphersuite()]).await.unwrap();

                    alice_central
                        .new_conversation(id.clone(), case.credential_type, case.cfg.clone())
                        .await
                        .unwrap();
                    alice_central
                        .invite(&id, &mut bob_central, case.custom_cfg())
                        .await
                        .unwrap();

                    // Create another central which will be desynchronized at some point
                    let mut alice_central_mirror = MlsCentral::try_new(alice_cfg).await.unwrap();
                    assert!(alice_central_mirror.try_talk_to(&id, &mut bob_central).await.is_ok());

                    // alice original instance will update its key without synchronizing with its mirror
                    let commit = alice_central.update_keying_material(&id).await.unwrap().commit;
                    alice_central.commit_accepted(&id).await.unwrap();
                    // at this point using mirror instance is unsafe since it will erase the other
                    // instance state in keystore...
                    bob_central
                        .decrypt_message(&id, commit.to_bytes().unwrap())
                        .await
                        .unwrap();
                    // so here we cannot test that mirror instance can talk to Bob because it would
                    // mess up the test, but trust me, it does !

                    // after restoring from disk, mirror instance got the right key material for
                    // the current epoch hence can talk to Bob
                    alice_central_mirror.restore_from_disk().await.unwrap();
                    assert!(alice_central_mirror.try_talk_to(&id, &mut bob_central).await.is_ok());
                })
            })
            .await
        }
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn can_fetch_client_public_key(case: TestCase) {
        run_tests(move |[tmp_dir_argument]| {
            Box::pin(async move {
                let configuration = MlsCentralConfiguration::try_new(
                    tmp_dir_argument,
                    "test".to_string(),
                    Some("potato".into()),
                    vec![case.ciphersuite()],
                    None,
                )
                .unwrap();

                let result = MlsCentral::try_new(configuration.clone()).await;
                assert!(result.is_ok());
            })
        })
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn can_2_phase_init_central(case: TestCase) {
        run_tests(move |[tmp_dir_argument]| {
            Box::pin(async move {
                let configuration = MlsCentralConfiguration::try_new(
                    tmp_dir_argument,
                    "test".to_string(),
                    None,
                    vec![case.ciphersuite()],
                    None,
                )
                .unwrap();
                // phase 1: init without mls_client
                let mut central = MlsCentral::try_new(configuration).await.unwrap();
                assert!(central.mls_client.is_none());
                // phase 2: init mls_client
                let client_id = "alice".into();
                let identifier = match case.credential_type {
                    MlsCredentialType::Basic => ClientIdentifier::Basic(client_id),
                    MlsCredentialType::X509 => CertificateBundle::rand_identifier(&[case.ciphersuite()], client_id),
                };
                central.mls_init(identifier, vec![case.ciphersuite()]).await.unwrap();
                assert!(central.mls_client.is_some());
                // expect mls_client to work
                assert_eq!(
                    central
                        .get_or_create_client_keypackages(case.ciphersuite(), 2)
                        .await
                        .unwrap()
                        .len(),
                    2
                );
            })
        })
        .await
    }
}
