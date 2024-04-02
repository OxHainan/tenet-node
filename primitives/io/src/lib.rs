#![allow(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(not(feature = "std"), feature(alloc_error_handler))]
#[cfg(feature = "std")]
use aes_gcm::{
	aead::{AeadMut, Payload},
	Aes256Gcm, KeyInit, Nonce,
};
use scale_codec::{Decode, Encode};
#[cfg(feature = "std")]
use secp256k1::{ecdh::SharedSecret, PublicKey, SecretKey};
use sp_runtime_interface::runtime_interface;
use sp_std::vec::Vec;
/// Error aes gcm
#[derive(Debug, Encode, Decode)]
pub enum AesError {
	/// Bad shard key
	BadSharedKey,

	/// Bad shared key length
	BadSharedKeyLength,

	/// Bad encrypted
	BadEncrypted,

	/// Bad decrypted
	BadDecrypted,

	BadKeyLength,
}

/// Error verifying ECDSA signature
#[derive(Encode, Decode)]
pub enum EcdsaVerifyError {
	/// Invalid  public key
	BadPublicKey,

	/// Invalid secret key
	BadSecretKey,

	/// Invalid shared secret
	BadSharedSecret,
}

#[runtime_interface]
pub trait Crypto {
	/// encrypted
	fn encrypted(msg: &[u8], aad: &[u8; 32], pubkey: &[u8; 64]) -> Result<Vec<u8>, AesError> {
		let shared_key = shared_secret(pubkey).map_err(|_| AesError::BadSharedKey)?;
		Aes256Gcm::new_from_slice(&shared_key)
			.map_err(|_| AesError::BadSharedKeyLength)?
			.encrypt(Nonce::from_slice(&aad[20..]), Payload { aad, msg })
			.map_err(|_| AesError::BadEncrypted)
	}

	/// decrypted
	fn decrypted(msg: &[u8], aad: &[u8; 32], pubkey: &[u8; 64]) -> Result<Vec<u8>, AesError> {
		let shared_key = shared_secret(pubkey).map_err(|_| AesError::BadSharedKey)?;
		Aes256Gcm::new_from_slice(&shared_key)
			.map_err(|_| AesError::BadSharedKeyLength)?
			.decrypt(Nonce::from_slice(&aad[20..]), Payload { aad, msg })
			.map_err(|_| AesError::BadDecrypted)
	}

	/// ecdh
	fn shared_secret(pubkey: &[u8; 64]) -> Result<[u8; 32], EcdsaVerifyError> {
		let mut tagged_full = [0u8; 65];
		tagged_full[0] = 0x04;
		tagged_full[1..].copy_from_slice(pubkey);
		let sk = <SecretKey as core::str::FromStr>::from_str(
			"57f0148f94d13095cfda539d0da0d1541304b678d8b36e243980aab4e1b7cead",
		)
		.map_err(|_| EcdsaVerifyError::BadSecretKey)?;
		Ok(SharedSecret::new(
			&PublicKey::from_slice(&tagged_full).map_err(|_| EcdsaVerifyError::BadPublicKey)?,
			&sk,
		)
		.secret_bytes())
	}

	// encrypt
	fn encrypt(msg: &[u8], nonce: &[u8]) -> Result<Vec<u8>, AesError> {
		Aes256Gcm::new_from_slice(&sp_core::hashing::keccak_256(b"12"))
			.map_err(|_| AesError::BadKeyLength)?
			.encrypt(
				Nonce::from_slice(&sp_core::hashing::keccak_256(nonce)[20..]),
				msg,
			)
			.map_err(|_| AesError::BadEncrypted)
	}

	// decrypt
	fn decrypt(msg: &[u8], nonce: &[u8]) -> Result<Vec<u8>, AesError> {
		Aes256Gcm::new_from_slice(&sp_core::hashing::keccak_256(b"12"))
			.map_err(|_| AesError::BadKeyLength)?
			.decrypt(
				Nonce::from_slice(&sp_core::hashing::keccak_256(nonce)[20..]),
				msg,
			)
			.map_err(|_| AesError::BadDecrypted)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_io::hashing;

	#[test]
	fn test_aes_gcm() {
		let pk = array_bytes::hex2array_unchecked("8db55b05db86c0b1786ca49f095d76344c9e6056b2f02701a7e7f3c20aabfd913ebbe148dd17c56551a52952371071a6c604b3f3abe8f2c8fa742158ea6dd7d4");

		assert!(crypto::shared_secret(&pk).is_ok());

		let msg = b"hello world";
		let aad = hashing::keccak_256(msg);
		let ciphertext = match crypto::encrypted(msg, &aad, &pk) {
			Ok(cipher) => cipher,
			Err(_) => Default::default(),
		};

		let plaintext = match crypto::decrypted(&ciphertext, &aad, &pk) {
			Ok(plain) => plain,
			Err(_) => Default::default(),
		};

		assert_eq!(&plaintext, msg);
	}
}
