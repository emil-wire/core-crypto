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

use crate::clients::{EmulatedClient, EmulatedClientProtocol, EmulatedClientType, EmulatedMlsClient};
use color_eyre::eyre::Result;
use core_crypto::prelude::tls_codec::Serialize;
use core_crypto::prelude::*;

#[derive(Debug)]
pub struct CoreCryptoNativeClient {
    cc: CoreCrypto,
    client_id: Vec<u8>,
    #[cfg(feature = "proteus")]
    prekey_last_id: u16,
}

impl CoreCryptoNativeClient {
    pub async fn new() -> Result<Self> {
        let client_id = uuid::Uuid::new_v4();

        let ciphersuites = vec![MlsCiphersuite::default()];
        let configuration = MlsCentralConfiguration::try_new(
            "whatever".into(),
            "test".into(),
            Some(client_id.as_hyphenated().to_string().as_bytes().into()),
            ciphersuites,
        )?;

        let cc = CoreCrypto::from(MlsCentral::try_new_in_memory(configuration, None).await?);

        Ok(Self {
            cc,
            client_id: client_id.into_bytes().into(),
            #[cfg(feature = "proteus")]
            prekey_last_id: 0,
        })
    }
}

#[async_trait::async_trait(?Send)]
impl EmulatedClient for CoreCryptoNativeClient {
    fn client_name(&self) -> &str {
        "CoreCrypto::native"
    }

    fn client_type(&self) -> EmulatedClientType {
        EmulatedClientType::Native
    }

    fn client_id(&self) -> &[u8] {
        self.client_id.as_slice()
    }

    fn client_protocol(&self) -> EmulatedClientProtocol {
        EmulatedClientProtocol::MLS | EmulatedClientProtocol::PROTEUS
    }

    async fn wipe(mut self) -> Result<()> {
        self.cc.take().wipe().await?;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl EmulatedMlsClient for CoreCryptoNativeClient {
    async fn get_keypackage(&mut self) -> Result<Vec<u8>> {
        let kps = self.cc.client_keypackages(1).await?;
        Ok(kps[0].key_package().tls_serialize_detached()?)
    }

    async fn add_client(&mut self, conversation_id: &[u8], client_id: &[u8], kp: &[u8]) -> Result<Vec<u8>> {
        if !self.cc.conversation_exists(&conversation_id.to_vec()).await {
            self.cc
                .new_conversation(conversation_id.to_vec(), Default::default())
                .await?;
        }

        let member = ConversationMember::new_raw(client_id.to_vec().into(), kp.to_vec())?;
        let welcome = self
            .cc
            .add_members_to_conversation(&conversation_id.to_vec(), &mut [member])
            .await?;

        Ok(welcome.welcome.tls_serialize_detached()?)
    }

    async fn kick_client(&mut self, conversation_id: &[u8], client_id: &[u8]) -> Result<Vec<u8>> {
        let commit = self
            .cc
            .remove_members_from_conversation(&conversation_id.to_vec(), &[client_id.to_vec().into()])
            .await?;

        Ok(commit.commit.to_bytes()?)
    }

    async fn process_welcome(&mut self, welcome: &[u8]) -> Result<Vec<u8>> {
        Ok(self
            .cc
            .process_raw_welcome_message(welcome.into(), MlsCustomConfiguration::default())
            .await?)
    }

    async fn encrypt_message(&mut self, conversation_id: &[u8], message: &[u8]) -> Result<Vec<u8>> {
        Ok(self.cc.encrypt_message(&conversation_id.to_vec(), message).await?)
    }

    async fn decrypt_message(&mut self, conversation_id: &[u8], message: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self
            .cc
            .decrypt_message(&conversation_id.to_vec(), message)
            .await?
            .app_msg)
    }
}

#[cfg(feature = "proteus")]
#[async_trait::async_trait(?Send)]
impl crate::clients::EmulatedProteusClient for CoreCryptoNativeClient {
    async fn init(&mut self) -> Result<()> {
        Ok(self.cc.proteus_init().await?)
    }

    async fn get_prekey(&mut self) -> Result<Vec<u8>> {
        self.prekey_last_id += 1;
        Ok(self.cc.proteus_new_prekey(self.prekey_last_id).await?)
    }

    async fn session_from_prekey(&mut self, session_id: &str, prekey: &[u8]) -> Result<()> {
        let _ = self.cc.proteus_session_from_prekey(session_id, prekey).await?;
        Ok(())
    }

    async fn session_from_message(&mut self, session_id: &str, message: &[u8]) -> Result<Vec<u8>> {
        let (_, ret) = self.cc.proteus_session_from_message(session_id, message).await?;
        Ok(ret)
    }

    async fn encrypt(&mut self, session_id: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        Ok(self.cc.proteus_encrypt(session_id, plaintext).await?)
    }

    async fn decrypt(&mut self, session_id: &str, ciphertext: &[u8]) -> Result<Vec<u8>> {
        Ok(self.cc.proteus_decrypt(session_id, ciphertext).await?)
    }

    async fn fingerprint(&self) -> Result<String> {
        Ok(self.cc.proteus_fingerprint()?)
    }
}

#[async_trait::async_trait(?Send)]
impl crate::clients::EmulatedE2eIdentityClient for CoreCryptoNativeClient {
    async fn new_acme_enrollment(&mut self, ciphersuite: MlsCiphersuite) -> Result<()> {
        let enrollment = self.cc.new_acme_enrollment(ciphersuite)?;
        let directory = serde_json::json!({
            "newNonce": "https://example.com/acme/new-nonce",
            "newAccount": "https://example.com/acme/new-account",
            "newOrder": "https://example.com/acme/new-order"
        });
        let directory = serde_json::to_vec(&directory)?;
        let directory = enrollment.directory_response(directory)?;
        let previous_nonce = "dmVQallIV29ZZkcwVkNLQTRKbG9HcVdyTWU5WEszdTE";

        enrollment.new_account_request(directory, previous_nonce.to_string())?;

        let account = serde_json::json!({
            "status": "valid",
            "contact": [
                "mailto:cert-admin@example.org",
                "mailto:admin@example.org"
            ],
            "orders": "https://example.com/acme/acct/evOfKhNU60wg/orders"
        });
        let account = serde_json::to_vec(&account)?;
        let _account = enrollment.new_account_response(account)?;
        Ok(())
    }
}
