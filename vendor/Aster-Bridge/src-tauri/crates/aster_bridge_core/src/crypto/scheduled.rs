//
// Aster Communications Inc.
//
// Copyright (c) 2026 Aster Communications Inc.
//
// This file is part of this project.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.
//
use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};

use crate::error::{BridgeError, Result};

const SCHEDULED_KEY_VERSION: &str = "astermail-scheduled-v1";
const NONCE_LEN: usize = 12;

fn derive_key(identity_key: &str) -> Zeroizing<[u8; 32]> {
    let mut material = Vec::with_capacity(identity_key.len() + SCHEDULED_KEY_VERSION.len());
    material.extend_from_slice(identity_key.as_bytes());
    material.extend_from_slice(SCHEDULED_KEY_VERSION.as_bytes());
    let digest = Sha256::digest(&material);
    material.zeroize();
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&digest);
    key
}

fn encrypt_with_key(key: &[u8; 32], plaintext: &[u8]) -> Result<(String, String)> {
    use rand_core::{OsRng, RngCore};
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| BridgeError::Crypto(format!("scheduled cipher init: {e}")))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| BridgeError::Crypto("scheduled encrypt failed".to_string()))?;
    Ok((STANDARD.encode(ciphertext), STANDARD.encode(nonce_bytes)))
}

fn decrypt_with_key(key: &[u8; 32], encrypted_b64: &str, nonce_b64: &str) -> Result<Vec<u8>> {
    let ciphertext = STANDARD
        .decode(encrypted_b64)
        .map_err(|e| BridgeError::Crypto(format!("scheduled data decode: {e}")))?;
    let nonce_bytes = STANDARD
        .decode(nonce_b64)
        .map_err(|e| BridgeError::Crypto(format!("scheduled nonce decode: {e}")))?;
    if nonce_bytes.len() != NONCE_LEN {
        return Err(BridgeError::Crypto("invalid scheduled nonce length".to_string()));
    }
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|e| BridgeError::Crypto(format!("scheduled cipher init: {e}")))?;
    cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext.as_ref())
        .map_err(|_| BridgeError::Crypto("scheduled decrypt failed".to_string()))
}

pub fn encrypt_local(identity_key: &str, plaintext: &[u8]) -> Result<(String, String)> {
    let key = derive_key(identity_key);
    encrypt_with_key(&key, plaintext)
}

pub fn decrypt_local(
    identity_key: &str,
    previous_keys: &[String],
    encrypted_b64: &str,
    nonce_b64: &str,
) -> Result<Vec<u8>> {
    let key = derive_key(identity_key);
    if let Ok(value) = decrypt_with_key(&key, encrypted_b64, nonce_b64) {
        return Ok(value);
    }
    for previous in previous_keys {
        let key = derive_key(previous);
        if let Ok(value) = decrypt_with_key(&key, encrypted_b64, nonce_b64) {
            return Ok(value);
        }
    }
    Err(BridgeError::Crypto("scheduled decrypt failed".to_string()))
}

pub fn encrypt_ephemeral(plaintext: &[u8]) -> Result<(String, String, String, String)> {
    use rand_core::{OsRng, RngCore};
    let mut raw_key = Zeroizing::new([0u8; 32]);
    OsRng.fill_bytes(raw_key.as_mut());
    let (ciphertext, nonce) = encrypt_with_key(&raw_key, plaintext)?;
    let mut base_nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut base_nonce);
    Ok((
        ciphertext,
        nonce,
        STANDARD.encode(raw_key.as_slice()),
        STANDARD.encode(base_nonce),
    ))
}

pub fn decrypt_ephemeral(encrypted_b64: &str, nonce_b64: &str, ephemeral_key_b64: &str) -> Result<Vec<u8>> {
    let raw = STANDARD
        .decode(ephemeral_key_b64)
        .map_err(|e| BridgeError::Crypto(format!("scheduled ephemeral key decode: {e}")))?;
    if raw.len() != 32 {
        return Err(BridgeError::Crypto("invalid scheduled ephemeral key".to_string()));
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&raw);
    decrypt_with_key(&key, encrypted_b64, nonce_b64)
}
