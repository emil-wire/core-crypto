// This file was autogenerated by some hot garbage in the `uniffi` crate.
// Trust me, you don't want to mess with it!

#pragma once

#include <stdbool.h>
#include <stdint.h>

// The following structs are used to implement the lowest level
// of the FFI, and thus useful to multiple uniffied crates.
// We ensure they are declared exactly once, with a header guard, UNIFFI_SHARED_H.
#ifdef UNIFFI_SHARED_H
    // We also try to prevent mixing versions of shared uniffi header structs.
    // If you add anything to the #else block, you must increment the version suffix in UNIFFI_SHARED_HEADER_V4
    #ifndef UNIFFI_SHARED_HEADER_V4
        #error Combining helper code from multiple versions of uniffi is not supported
    #endif // ndef UNIFFI_SHARED_HEADER_V4
#else
#define UNIFFI_SHARED_H
#define UNIFFI_SHARED_HEADER_V4
// ⚠️ Attention: If you change this #else block (ending in `#endif // def UNIFFI_SHARED_H`) you *must* ⚠️
// ⚠️ increment the version suffix in all instances of UNIFFI_SHARED_HEADER_V4 in this file.           ⚠️

typedef struct RustBuffer
{
    int32_t capacity;
    int32_t len;
    uint8_t *_Nullable data;
} RustBuffer;

typedef int32_t (*ForeignCallback)(uint64_t, int32_t, RustBuffer, RustBuffer *_Nonnull);

typedef struct ForeignBytes
{
    int32_t len;
    const uint8_t *_Nullable data;
} ForeignBytes;

// Error definitions
typedef struct RustCallStatus {
    int8_t code;
    RustBuffer errorBuf;
} RustCallStatus;

// ⚠️ Attention: If you change this #else block (ending in `#endif // def UNIFFI_SHARED_H`) you *must* ⚠️
// ⚠️ increment the version suffix in all instances of UNIFFI_SHARED_HEADER_V4 in this file.           ⚠️
#endif // def UNIFFI_SHARED_H

void ffi_CoreCrypto_bbb3_CoreCrypto_object_free(
      void*_Nonnull ptr,
    RustCallStatus *_Nonnull out_status
    );
void*_Nonnull CoreCrypto_bbb3_CoreCrypto_new(
      RustBuffer path,RustBuffer key,RustBuffer client_id,RustBuffer entropy_seed,
    RustCallStatus *_Nonnull out_status
    );
void CoreCrypto_bbb3_CoreCrypto_set_callbacks(
      void*_Nonnull ptr,uint64_t callbacks,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_client_public_key(
      void*_Nonnull ptr,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_client_keypackages(
      void*_Nonnull ptr,uint32_t amount_requested,
    RustCallStatus *_Nonnull out_status
    );
uint64_t CoreCrypto_bbb3_CoreCrypto_client_valid_keypackages_count(
      void*_Nonnull ptr,
    RustCallStatus *_Nonnull out_status
    );
void CoreCrypto_bbb3_CoreCrypto_create_conversation(
      void*_Nonnull ptr,RustBuffer conversation_id,RustBuffer config,
    RustCallStatus *_Nonnull out_status
    );
int8_t CoreCrypto_bbb3_CoreCrypto_conversation_exists(
      void*_Nonnull ptr,RustBuffer conversation_id,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_process_welcome_message(
      void*_Nonnull ptr,RustBuffer welcome_message,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_add_clients_to_conversation(
      void*_Nonnull ptr,RustBuffer conversation_id,RustBuffer clients,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_remove_clients_from_conversation(
      void*_Nonnull ptr,RustBuffer conversation_id,RustBuffer clients,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_decrypt_message(
      void*_Nonnull ptr,RustBuffer conversation_id,RustBuffer payload,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_encrypt_message(
      void*_Nonnull ptr,RustBuffer conversation_id,RustBuffer message,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_new_add_proposal(
      void*_Nonnull ptr,RustBuffer conversation_id,RustBuffer key_package,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_new_update_proposal(
      void*_Nonnull ptr,RustBuffer conversation_id,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_new_remove_proposal(
      void*_Nonnull ptr,RustBuffer conversation_id,RustBuffer client_id,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_new_external_add_proposal(
      void*_Nonnull ptr,RustBuffer conversation_id,uint64_t epoch,RustBuffer key_package,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_new_external_remove_proposal(
      void*_Nonnull ptr,RustBuffer conversation_id,uint64_t epoch,RustBuffer key_package_ref,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_update_keying_material(
      void*_Nonnull ptr,RustBuffer conversation_id,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_join_by_external_commit(
      void*_Nonnull ptr,RustBuffer group_state,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_export_group_state(
      void*_Nonnull ptr,RustBuffer conversation_id,
    RustCallStatus *_Nonnull out_status
    );
void CoreCrypto_bbb3_CoreCrypto_merge_pending_group_from_external_commit(
      void*_Nonnull ptr,RustBuffer conversation_id,RustBuffer config,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_random_bytes(
      void*_Nonnull ptr,uint32_t length,
    RustCallStatus *_Nonnull out_status
    );
void CoreCrypto_bbb3_CoreCrypto_reseed_rng(
      void*_Nonnull ptr,RustBuffer seed,
    RustCallStatus *_Nonnull out_status
    );
void CoreCrypto_bbb3_CoreCrypto_commit_accepted(
      void*_Nonnull ptr,RustBuffer conversation_id,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_CoreCrypto_commit_pending_proposals(
      void*_Nonnull ptr,RustBuffer conversation_id,
    RustCallStatus *_Nonnull out_status
    );
void ffi_CoreCrypto_bbb3_CoreCryptoCallbacks_init_callback(
      ForeignCallback  _Nonnull callback_stub,
    RustCallStatus *_Nonnull out_status
    );
void*_Nonnull CoreCrypto_bbb3_init_with_path_and_key(
      RustBuffer path,RustBuffer key,RustBuffer client_id,RustBuffer entropy_seed,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer CoreCrypto_bbb3_version(
      
    RustCallStatus *_Nonnull out_status
    );
RustBuffer ffi_CoreCrypto_bbb3_rustbuffer_alloc(
      int32_t size,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer ffi_CoreCrypto_bbb3_rustbuffer_from_bytes(
      ForeignBytes bytes,
    RustCallStatus *_Nonnull out_status
    );
void ffi_CoreCrypto_bbb3_rustbuffer_free(
      RustBuffer buf,
    RustCallStatus *_Nonnull out_status
    );
RustBuffer ffi_CoreCrypto_bbb3_rustbuffer_reserve(
      RustBuffer buf,int32_t additional,
    RustCallStatus *_Nonnull out_status
    );
