use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::RngCore;
use secp256k1::ecdh::SharedSecret;
use secp256k1::{PublicKey, SecretKey};

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

fn xonly_to_compressed(xonly_bytes: &[u8]) -> Result<[u8; 33], String> {
    if xonly_bytes.len() != 32 {
        return Err("expected 32-byte x-only pubkey".to_string());
    }
    let mut compressed = [0u8; 33];
    compressed[0] = 0x02;
    compressed[1..].copy_from_slice(xonly_bytes);
    Ok(compressed)
}

pub fn encrypt(
    sender_sk: &[u8; 32],
    recipient_pk_hex: &str,
    plaintext: &[u8],
) -> Result<String, String> {
    let secret = SecretKey::from_slice(sender_sk).map_err(|e| format!("bad secret: {e}"))?;
    let recipient_pk_bytes = hex::decode(recipient_pk_hex).map_err(|e| format!("bad pubkey hex: {e}"))?;
    let compressed = xonly_to_compressed(&recipient_pk_bytes)?;
    let recipient_pk = PublicKey::from_slice(&compressed).map_err(|e| format!("bad pubkey: {e}"))?;

    let shared = SharedSecret::new(&recipient_pk, &secret);
    let key: [u8; 32] = shared.secret_bytes();

    let mut iv = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut iv);

    let mut buf = vec![0u8; plaintext.len() + 16];
    buf[..plaintext.len()].copy_from_slice(plaintext);
    let encrypted = Aes256CbcEnc::new(&key.into(), &iv.into())
        .encrypt_padded_mut::<Pkcs7>(buf.as_mut_slice(), plaintext.len())
        .map_err(|e| format!("AES encrypt failed: {e}"))?;

    let iv_b64 = B64.encode(iv);
    let text_b64 = B64.encode(encrypted);
    Ok(format!("{text_b64}?iv={iv_b64}"))
}

pub fn decrypt(
    recipient_sk: &[u8; 32],
    sender_pk_hex: &str,
    encrypted: &str,
) -> Result<Vec<u8>, String> {
    let secret = SecretKey::from_slice(recipient_sk).map_err(|e| format!("bad secret: {e}"))?;
    let sender_pk_bytes = hex::decode(sender_pk_hex).map_err(|e| format!("bad pubkey hex: {e}"))?;
    let compressed = xonly_to_compressed(&sender_pk_bytes)?;
    let sender_pk = PublicKey::from_slice(&compressed).map_err(|e| format!("bad pubkey: {e}"))?;

    let shared = SharedSecret::new(&sender_pk, &secret);
    let key: [u8; 32] = shared.secret_bytes();

    let (text_b64, iv_b64) = encrypted
        .split_once("?iv=")
        .ok_or("missing ?iv= separator")?;

    let iv = B64.decode(iv_b64).map_err(|e| format!("bad iv b64: {e}"))?;
    let ciphertext = B64.decode(text_b64).map_err(|e| format!("bad text b64: {e}"))?;
    if iv.len() != 16 {
        return Err("iv must be 16 bytes".to_string());
    }
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err("invalid ciphertext length".to_string());
    }
    let iv_arr: [u8; 16] = iv.try_into().map_err(|_| "iv must be 16 bytes".to_string())?;

    let mut buf = ciphertext.clone();
    let decrypted = Aes256CbcDec::new(&key.into(), &iv_arr.into())
        .decrypt_padded_mut::<Pkcs7>(buf.as_mut_slice())
        .map_err(|e| format!("AES decrypt failed: {e}"))?;

    Ok(decrypted.to_vec())
}