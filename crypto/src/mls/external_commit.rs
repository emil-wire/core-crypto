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

use openmls::prelude::{MlsGroup, MlsMessageOut, Proposal, Sender, StagedCommit, VerifiablePublicGroupState};
use openmls_traits::{crypto::OpenMlsCrypto, OpenMlsCryptoProvider};

use core_crypto_keystore::CryptoKeystoreMls;

use crate::{
    mls::{ConversationId, MlsCentral},
    prelude::{
        ClientId, MlsConversation, MlsConversationConfiguration, MlsCustomConfiguration, MlsPublicGroupStateBundle,
    },
    CoreCryptoCallbacks, CryptoError, CryptoResult, MlsError,
};

/// Returned when a commit is created
#[derive(Debug)]
pub struct MlsConversationInitBundle {
    /// Identifier of the conversation joined by external commit
    pub conversation_id: ConversationId,
    /// The external commit message
    pub commit: MlsMessageOut,
    /// [`PublicGroupState`] (aka GroupInfo) which becomes valid when the external commit is accepted by the Delivery Service
    pub public_group_state: MlsPublicGroupStateBundle,
}

impl MlsConversationInitBundle {
    /// Serializes both wrapped objects into TLS and return them as a tuple of byte arrays.
    /// 0 -> external commit
    /// 1 -> public group state
    #[allow(clippy::type_complexity)]
    pub fn to_bytes_pair(self) -> CryptoResult<(Vec<u8>, MlsPublicGroupStateBundle)> {
        let commit = self.commit.to_bytes().map_err(MlsError::from)?;
        Ok((commit, self.public_group_state))
    }
}

impl MlsCentral {
    /// Issues an external commit and stores the group in a temporary table. This method is
    /// intended for example when a new client wants to join the user's existing groups.
    /// On success this function will return the group id and a message to be fanned out to other
    /// clients.
    ///
    /// If the Delivery Service accepts the external commit, you have to [merge_pending_group_from_external_commit]
    /// in order to get back a functional MLS group. On the opposite, if it rejects it, you can either
    /// retry by just calling again [join_by_external_commit], no need to [clear_pending_group_from_external_commit].
    /// If you want to abort the operation (too many retries or the user decided to abort), you can use
    /// [clear_pending_group_from_external_commit] in order not to bloat the user's storage but nothing
    /// bad can happen if you forget to except some storage space wasted.
    ///
    /// # Arguments
    /// * `group_state` - a verifiable public group state. it can be obtained by deserializing a TLS
    /// serialized `PublicGroupState` object
    /// * `custom_cfg` - configuration of the MLS conversation fetched from the Delivery Service
    ///
    /// # Return type
    /// It will return a tuple with the group/conversation id and the message containing the
    /// commit that was generated by this call
    ///
    /// # Errors
    /// Errors resulting from OpenMls, the KeyStore calls and serialization
    pub async fn join_by_external_commit(
        &self,
        public_group_state: VerifiablePublicGroupState,
        custom_cfg: MlsCustomConfiguration,
    ) -> CryptoResult<MlsConversationInitBundle> {
        let credentials = self
            .mls_client
            .as_ref()
            .ok_or(CryptoError::MlsNotInitialized)?
            .credentials();

        let serialized_cfg = serde_json::to_vec(&custom_cfg).map_err(MlsError::MlsKeystoreSerializationError)?;

        let configuration = MlsConversationConfiguration {
            custom: custom_cfg,
            ..Default::default()
        };
        let (mut group, commit, pgs) = MlsGroup::join_by_external_commit(
            &self.mls_backend,
            None,
            public_group_state,
            &configuration.as_openmls_default_configuration()?,
            &[],
            credentials,
        )
        .await
        .map_err(MlsError::from)?;

        let mut group_serialized = vec![];
        group.save(&mut group_serialized)?;

        self.mls_backend
            .key_store()
            .mls_pending_groups_save(group.group_id().as_slice(), &group_serialized, &serialized_cfg)
            .await?;
        Ok(MlsConversationInitBundle {
            conversation_id: group.group_id().to_vec(),
            commit,
            public_group_state: MlsPublicGroupStateBundle::try_new_full_plaintext(pgs)?,
        })
    }

    /// This merges the commit generated by [join_by_external_commit], persists the group permanently and
    /// deletes the temporary one. After merging, the group should be fully functional.
    ///
    /// # Arguments
    /// * `id` - the conversation id
    ///
    /// # Errors
    /// Errors resulting from OpenMls, the KeyStore calls and deserialization
    pub async fn merge_pending_group_from_external_commit(&mut self, id: &ConversationId) -> CryptoResult<()> {
        // Retrieve the pending MLS group from the keystore
        let keystore = self.mls_backend.key_store();
        let (group, cfg) = keystore.mls_pending_groups_load(id).await?;
        let mut mls_group = MlsGroup::load(&mut &group[..])?;

        // Merge it aka bring the MLS group to life and make it usable
        mls_group.merge_pending_commit().map_err(MlsError::from)?;

        // Restore the custom configuration and build a conversation from it
        let custom_cfg = serde_json::from_slice(&cfg).map_err(MlsError::MlsKeystoreSerializationError)?;
        let configuration = MlsConversationConfiguration {
            custom: custom_cfg,
            ..Default::default()
        };

        // Persist the now usable MLS group in the keystore
        // TODO: find a way to make the insertion of the MlsGroup and deletion of the pending group transactional
        let conversation = MlsConversation::from_mls_group(mls_group, configuration, &self.mls_backend).await?;
        self.mls_groups.insert(id.clone(), conversation);

        // cleanup the pending group we no longer need
        keystore.mls_pending_groups_delete(id).await?;
        Ok(())
    }

    /// In case the external commit generated by [join_by_external_commit] is rejected by the Delivery Service
    /// and we want to abort this external commit once for all, we can wipe out the pending group from
    /// the keystore in order not to waste space
    ///
    /// # Arguments
    /// * `id` - the conversation id
    ///
    /// # Errors
    /// Errors resulting from the KeyStore calls
    pub async fn clear_pending_group_from_external_commit(&mut self, id: &ConversationId) -> CryptoResult<()> {
        Ok(self.mls_backend.key_store().mls_pending_groups_delete(id).await?)
    }
}

impl MlsConversation {
    pub(crate) async fn validate_external_commit(
        &self,
        commit: &StagedCommit,
        sender: Option<ClientId>,
        callbacks: Option<&dyn CoreCryptoCallbacks>,
        backend: &impl OpenMlsCrypto,
    ) -> CryptoResult<()> {
        // i.e. has this commit been created by [MlsCentral::join_by_external_commit] ?
        let is_external_init = commit
            .staged_proposal_queue()
            .any(|p| matches!(p.sender(), Sender::NewMember) && matches!(p.proposal(), Proposal::ExternalInit(_)));

        if is_external_init {
            let callbacks = callbacks.ok_or(CryptoError::CallbacksNotSet)?;
            let sender = sender.ok_or(CryptoError::UnauthorizedExternalCommit)?;
            // first let's verify the sender belongs to an user already in the MLS group
            let existing_clients = self.members_in_next_epoch(backend);
            if !callbacks
                .client_is_existing_group_user(self.id.clone(), sender.clone(), existing_clients.clone())
                .await
            {
                return Err(CryptoError::UnauthorizedExternalCommit);
            }
            // then verify that the user this client belongs to has the right role (is allowed)
            // to perform such operation
            if !callbacks
                .user_authorize(self.id.clone(), sender, existing_clients)
                .await
            {
                return Err(CryptoError::UnauthorizedExternalCommit);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{prelude::MlsConversationInitBundle, test_utils::*, CryptoError};
    use openmls::prelude::*;
    use wasm_bindgen_test::*;

    use core_crypto_keystore::{CryptoKeystoreError, CryptoKeystoreMls, MissingKeyErrorKind};

    wasm_bindgen_test_configure!(run_in_browser);

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn join_by_external_commit_should_succeed(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, mut bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();
                    alice_central
                        .new_conversation(id.clone(), case.cfg.clone())
                        .await
                        .unwrap();

                    // export Alice group info
                    let public_group_state = alice_central.verifiable_public_group_state(&id).await;

                    // Bob tries to join Alice's group
                    let MlsConversationInitBundle {
                        conversation_id: group_id,
                        commit: external_commit,
                        ..
                    } = bob_central
                        .join_by_external_commit(public_group_state, case.custom_cfg())
                        .await
                        .unwrap();
                    assert_eq!(group_id.as_slice(), &id);

                    // Alice acks the request and adds the new member
                    assert_eq!(alice_central[&id].members().len(), 1);
                    alice_central
                        .decrypt_message(&id, &external_commit.to_bytes().unwrap())
                        .await
                        .unwrap();
                    assert_eq!(alice_central[&id].members().len(), 2);

                    // Let's say backend accepted our external commit.
                    // So Bob can merge the commit and update the local state
                    assert!(bob_central.get_conversation(&id).is_err());
                    bob_central.merge_pending_group_from_external_commit(&id).await.unwrap();
                    assert!(bob_central.get_conversation(&id).is_ok());
                    assert_eq!(bob_central[&id].members().len(), 2);
                    assert!(alice_central.talk_to(&id, &mut bob_central).await.is_ok());

                    // Pending group removed from keystore
                    let error = alice_central.mls_backend.key_store().mls_pending_groups_load(&id).await;
                    assert!(matches!(
                        error.unwrap_err(),
                        CryptoKeystoreError::MissingKeyInStore(MissingKeyErrorKind::MlsPendingGroup)
                    ));

                    // Ensure it's durable i.e. MLS group has been persisted
                    bob_central.drop_and_restore(&group_id).await;
                    assert!(bob_central.talk_to(&id, &mut alice_central).await.is_ok());
                })
            },
        )
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn join_by_external_commit_should_be_retriable(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, mut bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();
                    alice_central
                        .new_conversation(id.clone(), case.cfg.clone())
                        .await
                        .unwrap();

                    // export Alice group info
                    let public_group_state = alice_central.verifiable_public_group_state(&id).await;

                    // Bob tries to join Alice's group
                    bob_central
                        .join_by_external_commit(public_group_state.clone(), case.custom_cfg())
                        .await
                        .unwrap();
                    // BUT for some reason the Delivery Service will reject this external commit
                    // e.g. another commit arrived meanwhile and the [PublicGroupState] is no longer valid

                    // Retrying
                    let MlsConversationInitBundle {
                        conversation_id,
                        commit: external_commit,
                        ..
                    } = bob_central
                        .join_by_external_commit(public_group_state, case.custom_cfg())
                        .await
                        .unwrap();
                    assert_eq!(conversation_id.as_slice(), &id);

                    // Alice decrypts the external commit and adds Bob
                    assert_eq!(alice_central[&id].members().len(), 1);
                    alice_central
                        .decrypt_message(&id, &external_commit.to_bytes().unwrap())
                        .await
                        .unwrap();
                    assert_eq!(alice_central[&id].members().len(), 2);

                    // And Bob can merge its external commit
                    bob_central.merge_pending_group_from_external_commit(&id).await.unwrap();
                    assert!(bob_central.get_conversation(&id).is_ok());
                    assert_eq!(bob_central[&id].members().len(), 2);
                    assert!(alice_central.talk_to(&id, &mut bob_central).await.is_ok());
                })
            },
        )
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn join_by_external_commit_should_fail_when_bad_epoch(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();
                    alice_central
                        .new_conversation(id.clone(), case.cfg.clone())
                        .await
                        .unwrap();

                    let public_group_state = alice_central.verifiable_public_group_state(&id).await;
                    // try to make an external join into Alice's group
                    let MlsConversationInitBundle {
                        commit: external_commit,
                        ..
                    } = bob_central
                        .join_by_external_commit(public_group_state, case.custom_cfg())
                        .await
                        .unwrap();

                    // Alice creates a new commit before receiving the external join
                    alice_central.update_keying_material(&id).await.unwrap();
                    alice_central.commit_accepted(&id).await.unwrap();

                    // receiving the external join with outdated epoch should fail because of
                    // the wrong epoch
                    let result = alice_central
                        .decrypt_message(&id, &external_commit.to_bytes().unwrap())
                        .await;
                    assert!(matches!(result.unwrap_err(), crate::CryptoError::WrongEpoch));
                })
            },
        )
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn existing_clients_can_join_by_external_commit(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, mut bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();
                    alice_central
                        .new_conversation(id.clone(), case.cfg.clone())
                        .await
                        .unwrap();
                    alice_central
                        .invite(&id, &mut bob_central, case.custom_cfg())
                        .await
                        .unwrap();
                    let public_group_state = alice_central.verifiable_public_group_state(&id).await;
                    // Alice can rejoin by external commit
                    let alice_join = alice_central
                        .join_by_external_commit(public_group_state.clone(), case.custom_cfg())
                        .await;
                    assert!(alice_join.is_ok());
                    // So can Bob
                    let bob_join = bob_central
                        .join_by_external_commit(public_group_state, case.custom_cfg())
                        .await;
                    assert!(bob_join.is_ok());
                })
            },
        )
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn join_by_external_commit_should_fail_when_no_pending_external_commit(case: TestCase) {
        run_test_with_central(case.clone(), move |[mut central]| {
            Box::pin(async move {
                let id = conversation_id();
                // try to merge an inexisting pending group
                let merge_unknown = central.merge_pending_group_from_external_commit(&id).await;

                assert!(matches!(
                    merge_unknown.unwrap_err(),
                    crate::CryptoError::KeyStoreError(CryptoKeystoreError::MissingKeyInStore(
                        MissingKeyErrorKind::MlsPendingGroup
                    ))
                ));
            })
        })
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn join_by_external_commit_should_return_valid_public_group_state(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob", "charlie"],
            move |[mut alice_central, mut bob_central, mut charlie_central]| {
                Box::pin(async move {
                    let id = conversation_id();
                    alice_central
                        .new_conversation(id.clone(), case.cfg.clone())
                        .await
                        .unwrap();

                    // export Alice group info
                    let public_group_state = alice_central.verifiable_public_group_state(&id).await;

                    // Bob tries to join Alice's group
                    let MlsConversationInitBundle {
                        commit: bob_external_commit,
                        public_group_state,
                        ..
                    } = bob_central
                        .join_by_external_commit(public_group_state, case.custom_cfg())
                        .await
                        .unwrap();

                    // Alice decrypts the commit, Bob's in !
                    alice_central
                        .decrypt_message(&id, &bob_external_commit.to_bytes().unwrap())
                        .await
                        .unwrap();
                    assert_eq!(alice_central[&id].members().len(), 2);

                    // Bob merges the commit, he's also in !
                    bob_central.merge_pending_group_from_external_commit(&id).await.unwrap();
                    assert!(bob_central.get_conversation(&id).is_ok());
                    assert_eq!(bob_central[&id].members().len(), 2);
                    assert!(alice_central.talk_to(&id, &mut bob_central).await.is_ok());

                    // Now charlie wants to join with the [PublicGroupState] from Bob's external commit
                    let bob_pgs = public_group_state.get_pgs();
                    let MlsConversationInitBundle {
                        commit: charlie_external_commit,
                        ..
                    } = charlie_central
                        .join_by_external_commit(bob_pgs, case.custom_cfg())
                        .await
                        .unwrap();

                    // Both Alice & Bob decrypt the commit
                    alice_central
                        .decrypt_message(&id, charlie_external_commit.to_bytes().unwrap())
                        .await
                        .unwrap();
                    bob_central
                        .decrypt_message(&id, charlie_external_commit.to_bytes().unwrap())
                        .await
                        .unwrap();
                    assert_eq!(alice_central[&id].members().len(), 3);
                    assert_eq!(bob_central[&id].members().len(), 3);

                    // Charlie merges the commit, he's also in !
                    charlie_central
                        .merge_pending_group_from_external_commit(&id)
                        .await
                        .unwrap();
                    assert!(charlie_central.get_conversation(&id).is_ok());
                    assert_eq!(charlie_central[&id].members().len(), 3);
                    assert!(charlie_central.talk_to(&id, &mut alice_central).await.is_ok());
                    assert!(charlie_central.talk_to(&id, &mut bob_central).await.is_ok());
                })
            },
        )
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn join_by_external_commit_should_fail_when_sender_user_not_in_group(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();

                    alice_central.callbacks(Box::new(ValidationCallbacks {
                        client_is_existing_group_user: false,
                        ..Default::default()
                    }));

                    alice_central
                        .new_conversation(id.clone(), case.cfg.clone())
                        .await
                        .unwrap();

                    // export Alice group info
                    let public_group_state = alice_central.verifiable_public_group_state(&id).await;

                    // Bob tries to join Alice's group
                    let MlsConversationInitBundle { commit, .. } = bob_central
                        .join_by_external_commit(public_group_state, case.custom_cfg())
                        .await
                        .unwrap();
                    let alice_accepts_ext_commit =
                        alice_central.decrypt_message(&id, &commit.to_bytes().unwrap()).await;
                    assert!(matches!(
                        alice_accepts_ext_commit.unwrap_err(),
                        CryptoError::UnauthorizedExternalCommit
                    ))
                })
            },
        )
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn join_by_external_commit_should_fail_when_sender_lacks_role(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();

                    alice_central.callbacks(Box::new(ValidationCallbacks {
                        user_authorize: false,
                        ..Default::default()
                    }));

                    alice_central
                        .new_conversation(id.clone(), case.cfg.clone())
                        .await
                        .unwrap();

                    // export Alice group info
                    let public_group_state = alice_central.verifiable_public_group_state(&id).await;

                    // Bob tries to join Alice's group
                    let MlsConversationInitBundle { commit, .. } = bob_central
                        .join_by_external_commit(public_group_state, case.custom_cfg())
                        .await
                        .unwrap();
                    let alice_accepts_ext_commit =
                        alice_central.decrypt_message(&id, &commit.to_bytes().unwrap()).await;
                    assert!(matches!(
                        alice_accepts_ext_commit.unwrap_err(),
                        CryptoError::UnauthorizedExternalCommit
                    ))
                })
            },
        )
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn clear_pending_group_should_succeed(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, mut bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();
                    alice_central
                        .new_conversation(id.clone(), case.cfg.clone())
                        .await
                        .unwrap();

                    // export Alice group info
                    let public_group_state = alice_central.verifiable_public_group_state(&id).await;

                    // Bob tries to join Alice's group
                    bob_central
                        .join_by_external_commit(public_group_state, case.custom_cfg())
                        .await
                        .unwrap();

                    // But for some reason, Bob wants to abort joining the group
                    bob_central.clear_pending_group_from_external_commit(&id).await.unwrap();

                    // Hence trying to merge the pending should fail
                    let result = bob_central.merge_pending_group_from_external_commit(&id).await;
                    assert!(matches!(
                        result.unwrap_err(),
                        CryptoError::KeyStoreError(CryptoKeystoreError::MissingKeyInStore(
                            MissingKeyErrorKind::MlsPendingGroup
                        ))
                    ))
                })
            },
        )
        .await
    }
}
