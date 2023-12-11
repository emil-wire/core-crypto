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

use crate::{
    connection::{DatabaseConnection, KeystoreDatabaseConnection},
    entities::{E2eiEnrollment, Entity, EntityBase, EntityFindParams, StringEntityId},
    CryptoKeystoreError, CryptoKeystoreResult, MissingKeyErrorKind,
};

#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
impl EntityBase for E2eiEnrollment {
    type ConnectionType = KeystoreDatabaseConnection;
    type AutoGeneratedFields = ();

    fn to_missing_key_err_kind() -> MissingKeyErrorKind {
        MissingKeyErrorKind::E2eiEnrollment
    }

    async fn find_all(_conn: &mut Self::ConnectionType, _params: EntityFindParams) -> CryptoKeystoreResult<Vec<Self>> {
        Err(CryptoKeystoreError::ImplementationError)
    }

    async fn save(&self, conn: &mut Self::ConnectionType) -> CryptoKeystoreResult<()> {
        let storage = conn.storage_mut();
        storage.save("e2ei_enrollment", &mut [self.clone()]).await?;
        Ok(())
    }

    async fn find_one(conn: &mut Self::ConnectionType, id: &StringEntityId) -> CryptoKeystoreResult<Option<Self>> {
        conn.storage().get("e2ei_enrollment", id.as_slice()).await
    }

    async fn count(_conn: &mut Self::ConnectionType) -> CryptoKeystoreResult<usize> {
        Err(CryptoKeystoreError::ImplementationError)
    }

    async fn delete(conn: &mut Self::ConnectionType, ids: &[StringEntityId]) -> CryptoKeystoreResult<()> {
        let storage = conn.storage_mut();
        let ids = ids.iter().map(StringEntityId::as_slice).collect::<Vec<_>>();
        storage.delete("e2ei_enrollment", &ids).await
    }
}

impl Entity for E2eiEnrollment {
    fn id_raw(&self) -> &[u8] {
        &self.id[..]
    }

    fn encrypt(&mut self, cipher: &aes_gcm::Aes256Gcm) -> CryptoKeystoreResult<()> {
        self.content = Self::encrypt_data(cipher, self.content.as_slice(), self.aad())?;
        Self::ConnectionType::check_buffer_size(self.content.len())?;
        Ok(())
    }

    fn decrypt(&mut self, cipher: &aes_gcm::Aes256Gcm) -> CryptoKeystoreResult<()> {
        self.content = Self::decrypt_data(cipher, self.content.as_slice(), self.aad())?;
        Ok(())
    }
}
