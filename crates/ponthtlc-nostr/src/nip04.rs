use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin::hashes::Hash;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use secp256k1::ecdh::SharedSecret;
use secp256k1::{PublicKey, SecretKey};

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

pub fn encrypt(
    sender_sk: &[u8; 32],
    recipient_pk_hex: &str,
    plaintext: &[u8],
) -> Result<String, String> {
    let secret = SecretKey::from_slice(sender_sk).map_err(|e| format!("bad secret: {e}"))?;
    let recipient_pk_bytes = hex::decode(recipient_pk_hex).map_err(|e| format!("bad pubkey hex: {e}"))?;
    let recipient_pk = PublicKey::from_slice(&recipient_pk_bytes).map_err(|e| format!("bad pubkey: {e}"))?;
    let shared = SharedSecret::new(&recipient_pk, &secret);
    let key = Sha256::hash(&shared.secret_bytes()).to_byte_array();

    let mut iv = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut iv);

    let mut buf = vec![0u8; plaintext.len() + 16];
    let encrypted = Aes256CbcEnc::new(&key.into(), &iv.into())
        .encrypt_padded_mut::<Pkcs7>(buf.as_mut_slice(), plaintext.len())
        .map_err(|e| format!("AES encrypt failed: {e}"))?;

    let iv_b64 = B64.encode(iv);
    let text_b64 = B64.encode(encrypted);
    Ok(format!("?iv={iv_b64}&text={text_b64}"))
}

pub fn decrypt(
    recipient_sk: &[u8; 32],
    sender_pk_hex: &str,
    encrypted: &str,
) -> Result<Vec<u8>, String> {
    let secret = SecretKey::from_slice(recipient_sk).map_err(|e| format!("bad secret: {e}"))?;
    let sender_pk_bytes = hex::decode(sender_pk_hex).map_err(|e| format!("bad pubkey hex: {e}"))?;
    let sender_pk = PublicKey::from_slice(&sender_pk_bytes).map_err(|e| format!("bad pubkey: {e}"))?;
    let shared = SharedSecret::new(&sender_pk, &secret);
    let key = Sha256::hash(&shared.secret_bytes()).to_byte_array();

    let params: std::collections::HashMap<&str, &str> = encrypted
        .trim_start_matches('?')
        .split('&')
        .filter_map(|kv| {
            let mut parts = kv.splitn(2, '=');
            Some((parts.next()?, parts.next()?))
        })
        .collect();

    let iv_b64 = params.get("iv").ok_or("missing iv")?;
    let text_b64 = params.get("text").ok_or("missing text")?;
    let iv = B64.decode(iv_b64).map_err(|e| format!("bad iv b64: {e}"))?;
    let ciphertext = B64.decode(text_b64).map_err(|e| format!("bad text b64: {e}"))?;
    if iv.len() != 16 {
        return Err("iv must be 16 bytes".to_string());
    }
    let iv_arr: [u8; 16] = iv.try_into().unwrap();

    let mut buf = ciphertext.clone();
    let decrypted = Aes256CbcDec::new(&key.into(), &iv_arr.into())
        .decrypt_padded_mut::<Pkcs7>(buf.as_mut_slice())
        .map_err(|e| format!("AES decrypt failed: {e}"))?;

    Ok(decrypted.to_vec())
}