// Copyright (C) 2024 Bellande Architecture Mechanism Research Innovation Center, Ronaldson Bellande

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};

#[derive(Debug)]
pub enum EncryptionError {
    InvalidKeyLength,
    InvalidDataFormat,
    EncryptionError,
    DecryptionError,
}

impl fmt::Display for EncryptionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EncryptionError::InvalidKeyLength => write!(f, "Invalid key length"),
            EncryptionError::InvalidDataFormat => write!(f, "Invalid data format"),
            EncryptionError::EncryptionError => write!(f, "Encryption error"),
            EncryptionError::DecryptionError => write!(f, "Decryption error"),
        }
    }
}

impl Error for EncryptionError {}

#[derive(Clone)]
pub struct PublicKey([u8; 32]);

pub struct PrivateKey([u8; 32]);

pub struct Signature([u8; 64]);

impl PublicKey {
    pub fn to_bytes(&self) -> [u8; 32] {
        self.0
    }
}

pub struct EncryptionManager {
    public_key: PublicKey,
    private_key: PrivateKey,
}

impl EncryptionManager {
    pub fn new() -> Self {
        let private_key = Self::generate_random_bytes();
        let public_key = Self::derive_public_key(&private_key);

        Self {
            public_key: PublicKey(public_key),
            private_key: PrivateKey(private_key),
        }
    }

    pub fn encrypt(&self, data: &[u8], shared_secret: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let key = self.derive_key(shared_secret)?;
        let nonce = Self::generate_random_bytes();

        let mut result = nonce.to_vec();
        result.extend_from_slice(data);

        for (i, byte) in result.iter_mut().enumerate().skip(12) {
            *byte ^= key[i % key.len()];
        }

        Ok(result)
    }

    pub fn decrypt(
        &self,
        encrypted_data: &[u8],
        shared_secret: &[u8],
    ) -> Result<Vec<u8>, EncryptionError> {
        if encrypted_data.len() < 12 {
            return Err(EncryptionError::InvalidDataFormat);
        }

        let key = self.derive_key(shared_secret)?;
        let mut decrypted = encrypted_data[12..].to_vec();

        for (i, byte) in decrypted.iter_mut().enumerate() {
            *byte ^= key[i % key.len()];
        }

        Ok(decrypted)
    }

    pub fn sign(&self, data: &[u8]) -> Signature {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        self.private_key.0.hash(&mut hasher);
        let hash = hasher.finish();

        let mut signature = [0u8; 64];
        signature[..8].copy_from_slice(&hash.to_be_bytes());
        Signature(signature)
    }

    pub fn verify(&self, public_key: &PublicKey, data: &[u8], signature: &Signature) -> bool {
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        public_key.0.hash(&mut hasher);
        let hash = hasher.finish();

        hash.to_be_bytes() == signature.0[..8]
    }

    pub fn public_key(&self) -> PublicKey {
        self.public_key.clone()
    }

    fn derive_key(&self, shared_secret: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        if shared_secret.len() < 32 {
            return Err(EncryptionError::InvalidKeyLength);
        }
        let mut hasher = DefaultHasher::new();
        shared_secret.hash(&mut hasher);
        let hash = hasher.finish();
        Ok(hash.to_be_bytes().to_vec())
    }

    fn derive_public_key(private_key: &[u8; 32]) -> [u8; 32] {
        let mut hasher = DefaultHasher::new();
        private_key.hash(&mut hasher);
        let hash = hasher.finish();
        let mut public_key = [0u8; 32];
        public_key[..8].copy_from_slice(&hash.to_be_bytes());
        public_key
    }

    fn generate_random_bytes() -> [u8; 32] {
        let mut bytes = [0u8; 32];
        for byte in &mut bytes {
            *byte = Self::generate_random_byte();
        }
        bytes
    }

    fn generate_random_byte() -> u8 {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .subsec_nanos();
        (nanos & 0xFF) as u8
    }
}

impl Default for EncryptionManager {
    fn default() -> Self {
        Self::new()
    }
}
