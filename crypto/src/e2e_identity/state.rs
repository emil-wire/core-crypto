use crate::{
    mls::credential::ext::CredentialExt,
    prelude::{ConversationId, CryptoResult, MlsCentral, MlsConversation},
};
use wire_e2e_identity::prelude::WireIdentityReader;

/// Indicates the state of a Conversation regarding end-to-end identity.
/// Note: this does not check pending state (pending commit, pending proposals) so it does not
/// consider members about to be added/removed
#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum E2eiConversationState {
    /// All clients have a valid E2EI certificate
    Verified,
    /// Some clients are either still Basic or their certificate is expired
    Degraded,
    /// All clients are still Basic. If all client have expired certificates, [E2eiConversationState::Degraded] is returned.
    NotEnabled,
}

impl MlsCentral {
    /// Indicates when to mark a conversation as degraded i.e. when not all its members have a X509
    /// Credential generated by Wire's end-to-end identity enrollment
    pub async fn e2ei_conversation_state(&mut self, id: &ConversationId) -> CryptoResult<E2eiConversationState> {
        Ok(self.get_conversation(id).await?.read().await.e2ei_conversation_state())
    }
}

impl MlsConversation {
    fn e2ei_conversation_state(&self) -> E2eiConversationState {
        let mut one_valid = false;
        let mut all_expired = true;

        let state = self
            .group
            .members()
            .fold(E2eiConversationState::Verified, |mut state, kp| {
                if let Ok(Some(cert)) = kp.credential.parse_leaf_cert() {
                    let invalid_identity = cert.extract_identity().is_err();

                    use openmls_x509_credential::X509Ext as _;
                    let is_time_valid = cert.is_time_valid().unwrap_or(false);
                    let is_time_invalid = !is_time_valid;
                    all_expired &= is_time_invalid;

                    let is_invalid = invalid_identity || is_time_invalid;
                    if is_invalid {
                        state = E2eiConversationState::Degraded;
                    } else {
                        one_valid = true
                    }
                } else {
                    all_expired = false;
                    state = E2eiConversationState::Degraded;
                };
                state
            });

        match (one_valid, all_expired) {
            (false, true) => E2eiConversationState::Degraded,
            (false, _) => E2eiConversationState::NotEnabled,
            _ => state,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::{
        e2e_identity::state::E2eiConversationState,
        mls::credential::tests::now,
        prelude::{CertificateBundle, Client, MlsCredentialType},
        test_utils::*,
    };
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    // testing the case where both Bob & Alice have the same Credential type
    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn uniform_conversation_should_be_degraded_when_basic(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, mut bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();

                    // That way the conversation creator (Alice) will have the same credential type as Bob
                    let creator_ct = case.credential_type;
                    alice_central
                        .new_conversation(&id, creator_ct, case.cfg.clone())
                        .await
                        .unwrap();
                    alice_central.invite_all(&case, &id, [&mut bob_central]).await.unwrap();

                    match case.credential_type {
                        MlsCredentialType::Basic => {
                            let alice_state = alice_central.e2ei_conversation_state(&id).await.unwrap();
                            let bob_state = bob_central.e2ei_conversation_state(&id).await.unwrap();
                            assert_eq!(alice_state, E2eiConversationState::NotEnabled);
                            assert_eq!(bob_state, E2eiConversationState::NotEnabled);
                        }
                        MlsCredentialType::X509 => {
                            let alice_state = alice_central.e2ei_conversation_state(&id).await.unwrap();
                            let bob_state = bob_central.e2ei_conversation_state(&id).await.unwrap();
                            assert_eq!(alice_state, E2eiConversationState::Verified);
                            assert_eq!(bob_state, E2eiConversationState::Verified);
                        }
                    }
                })
            },
        )
        .await
    }

    // testing the case where Bob & Alice have different Credential type
    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn heterogeneous_conversation_should_be_degraded(case: TestCase) {
        run_test_with_client_ids(
            case.clone(),
            ["alice", "bob"],
            move |[mut alice_central, mut bob_central]| {
                Box::pin(async move {
                    let id = conversation_id();

                    // That way the conversation creator (Alice) will have a different credential type than Bob
                    let creator_client = alice_central.mls_client.as_mut().unwrap();
                    let creator_ct = match case.credential_type {
                        MlsCredentialType::Basic => {
                            let cert_bundle = CertificateBundle::rand(
                                creator_client.id(),
                                case.cfg.ciphersuite.signature_algorithm(),
                            );
                            creator_client
                                .init_x509_credential_bundle_if_missing(
                                    &alice_central.mls_backend,
                                    case.signature_scheme(),
                                    cert_bundle,
                                )
                                .await
                                .unwrap();

                            MlsCredentialType::X509
                        }
                        MlsCredentialType::X509 => {
                            creator_client
                                .init_basic_credential_bundle_if_missing(
                                    &alice_central.mls_backend,
                                    case.signature_scheme(),
                                )
                                .await
                                .unwrap();
                            MlsCredentialType::Basic
                        }
                    };

                    alice_central
                        .new_conversation(&id, creator_ct, case.cfg.clone())
                        .await
                        .unwrap();
                    alice_central.invite_all(&case, &id, [&mut bob_central]).await.unwrap();

                    // since in that case both have a different credential type the conversation is always degraded
                    let alice_state = alice_central.e2ei_conversation_state(&id).await.unwrap();
                    let bob_state = bob_central.e2ei_conversation_state(&id).await.unwrap();
                    assert_eq!(alice_state, E2eiConversationState::Degraded);
                    assert_eq!(bob_state, E2eiConversationState::Degraded);
                })
            },
        )
        .await
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn should_be_degraded_when_one_expired(case: TestCase) {
        if case.is_x509() {
            run_test_with_client_ids(
                case.clone(),
                ["alice", "bob"],
                move |[mut alice_central, mut bob_central]| {
                    Box::pin(async move {
                        let id = conversation_id();

                        alice_central
                            .new_conversation(&id, case.credential_type, case.cfg.clone())
                            .await
                            .unwrap();
                        alice_central.invite_all(&case, &id, [&mut bob_central]).await.unwrap();

                        let expiration_time = core::time::Duration::from_secs(14);
                        let start = fluvio_wasm_timer::Instant::now();
                        let expiration = now() + expiration_time;

                        let builder = wire_e2e_identity::prelude::WireIdentityBuilder {
                            not_after: expiration,
                            ..Default::default()
                        };
                        let cert = CertificateBundle::new_from_builder(case.signature_scheme(), builder);
                        let cb = Client::new_x509_credential_bundle(cert).unwrap();
                        let commit = alice_central.e2ei_rotate(&id, &cb).await.unwrap().commit;
                        alice_central.commit_accepted(&id).await.unwrap();
                        bob_central
                            .decrypt_message(&id, commit.to_bytes().unwrap())
                            .await
                            .unwrap();

                        let elapsed = start.elapsed();
                        // Give time to the certificate to expire
                        if expiration_time > elapsed {
                            async_std::task::sleep(expiration_time - elapsed + core::time::Duration::from_secs(1))
                                .await;
                        }

                        let alice_state = alice_central.e2ei_conversation_state(&id).await.unwrap();
                        let bob_state = bob_central.e2ei_conversation_state(&id).await.unwrap();
                        assert_eq!(alice_state, E2eiConversationState::Degraded);
                        assert_eq!(bob_state, E2eiConversationState::Degraded);
                    })
                },
            )
            .await
        }
    }

    #[apply(all_cred_cipher)]
    #[wasm_bindgen_test]
    pub async fn should_be_degraded_when_all_expired(case: TestCase) {
        if case.is_x509() {
            run_test_with_client_ids(case.clone(), ["alice"], move |[mut alice_central]| {
                Box::pin(async move {
                    let id = conversation_id();

                    alice_central
                        .new_conversation(&id, case.credential_type, case.cfg.clone())
                        .await
                        .unwrap();

                    let expiration_time = core::time::Duration::from_secs(14);
                    let start = fluvio_wasm_timer::Instant::now();
                    let expiration = now() + expiration_time;

                    let builder = wire_e2e_identity::prelude::WireIdentityBuilder {
                        not_after: expiration,
                        ..Default::default()
                    };
                    let cert = CertificateBundle::new_from_builder(case.signature_scheme(), builder);
                    let cb = Client::new_x509_credential_bundle(cert).unwrap();
                    alice_central.e2ei_rotate(&id, &cb).await.unwrap();
                    alice_central.commit_accepted(&id).await.unwrap();

                    let elapsed = start.elapsed();
                    // Give time to the certificate to expire
                    if expiration_time > elapsed {
                        async_std::task::sleep(expiration_time - elapsed + core::time::Duration::from_secs(1)).await;
                    }

                    let alice_state = alice_central.e2ei_conversation_state(&id).await.unwrap();
                    assert_eq!(alice_state, E2eiConversationState::Degraded);
                })
            })
            .await
        }
    }
}
