//! `Secret.encrypt` / `decrypt`: AES-128-GCM with a random 12 byte IV in
//! front of the ciphertext and its 16 byte tag.
use aes_gcm::aead::{AeadCore, OsRng};
use aes_gcm::{AeadInPlace, Aes128Gcm, KeyInit, Nonce};

const NONCE_LEN: usize = 12;

pub struct Crypt {
    key: Aes128Gcm,
}

impl Crypt {
    pub fn new(secret: &[u8; 16]) -> Self {
        Self {
            key: Aes128Gcm::new_from_slice(secret).expect("16 byte AES key"),
        }
    }

    pub fn encrypt(&self, plain: &[u8]) -> Vec<u8> {
        let iv = Aes128Gcm::generate_nonce(&mut OsRng);
        let mut out = Vec::with_capacity(NONCE_LEN + plain.len() + 16);
        out.extend_from_slice(&iv);
        let mut body = plain.to_vec();
        self.key
            .encrypt_in_place(&iv, b"", &mut body)
            .expect("sealing a packet");
        out.extend_from_slice(&body);
        out
    }

    /// None for a packet that wasn't sealed with this secret.
    pub fn decrypt(&self, payload: &[u8]) -> Option<Vec<u8>> {
        if payload.len() < NONCE_LEN + 16 {
            return None;
        }
        let iv = Nonce::from_slice(&payload[..NONCE_LEN]);
        let mut body = payload[NONCE_LEN..].to_vec();
        self.key.decrypt_in_place(iv, b"", &mut body).ok()?;
        Some(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_rejects_other_keys() {
        let a = Crypt::new(&[7; 16]);
        let sealed = a.encrypt(b"hello voice");
        assert_eq!(sealed.len(), 12 + 11 + 16);
        assert_eq!(a.decrypt(&sealed).unwrap(), b"hello voice");
        assert!(Crypt::new(&[8; 16]).decrypt(&sealed).is_none());
    }

    /// NIST GCM test case 3 (AES-128, 96 bit IV): Java's
    /// `AES/GCM/NoPadding` gives the same bytes.
    #[test]
    fn matches_the_gcm_test_vector() {
        let key: [u8; 16] = hex("feffe9928665731c6d6a8f9467308308").try_into().unwrap();
        let iv = hex("cafebabefacedbaddecaf888");
        let pt = hex(concat!(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72",
            "1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255"
        ));
        let ct = hex(concat!(
            "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e",
            "21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091473f5985",
            "4d5c2af327cd64a62cf35abd2ba6fab4"
        ));
        let payload = [iv, ct].concat();
        assert_eq!(Crypt::new(&key).decrypt(&payload).unwrap(), pt);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
