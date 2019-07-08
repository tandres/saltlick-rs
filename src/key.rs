// Copyright (c) 2019, Nick Stevens <nick@bitcurry.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/license/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use lazy_static::lazy_static;
use pem;
use simple_asn1::{self, ASN1Block, ASN1Class, BigInt, BigUint, FromASN1, ToASN1, OID};
use sodiumoxide::crypto::box_::PublicKey as SodiumPublicKey;
use sodiumoxide::crypto::box_::SecretKey as SodiumSecretKey;

use crate::error::SaltlickError;

pub use sodiumoxide::crypto::box_::{self, PUBLICKEYBYTES, SECRETKEYBYTES};

lazy_static! {
    static ref CURVE25519_OID: OID = simple_asn1::oid!(1, 3, 101, 110);
}

/// Wrapper over libsodium-provided public key type.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct PublicKey {
    pub(crate) inner: SodiumPublicKey,
}

impl PublicKey {
    /// Load public key from raw Curve25519 bytes.
    ///
    /// This must be `PUBLICKEYBYTES` long. It corresponds to the key type used
    /// by libsodium 1.x.
    pub fn from_raw_curve25519(bytes: &[u8]) -> Result<PublicKey, SaltlickError> {
        let sodium_key =
            SodiumPublicKey::from_slice(bytes).ok_or(SaltlickError::IncorrectKeyLength)?;
        Ok(PublicKey { inner: sodium_key })
    }

    /// Load public key from PEM string.
    pub fn from_pem(pem: &str) -> Result<PublicKey, SaltlickError> {
        let pem::Pem { contents, .. } = pem::parse(pem)?;
        simple_asn1::der_decode::<Self>(&contents[..])
    }

    /// Export public key as PEM-encoded string.
    pub fn to_pem(&self) -> String {
        let der = simple_asn1::der_encode(self).expect("DER-encoding of PublicKey cannot fail");
        pem::encode(&pem::Pem {
            tag: String::from("PUBLIC KEY"),
            contents: der,
        })
    }
}

impl ToASN1 for PublicKey {
    type Error = SaltlickError;

    fn to_asn1_class(&self, _: ASN1Class) -> Result<Vec<ASN1Block>, Self::Error> {
        let mut public_key_asn1 = vec![];
        public_key_asn1.extend(Curve25519Algorithm.to_asn1()?);
        public_key_asn1.push(ASN1Block::BitString(
            0,
            PUBLICKEYBYTES * 8,
            Vec::from(&self.inner[..]),
        ));
        Ok(vec![ASN1Block::Sequence(0, public_key_asn1)])
    }
}

impl FromASN1 for PublicKey {
    type Error = SaltlickError;

    fn from_asn1(v: &[ASN1Block]) -> Result<(Self, &[ASN1Block]), Self::Error> {
        let key_seq = match v.get(0) {
            Some(ASN1Block::Sequence(_, key_seq)) => key_seq,
            _ => return Err(SaltlickError::InvalidKeyFormat),
        };

        // Only checks that algorithm is Curve25519, doesn't actually return the value.
        let algorithm_block = key_seq.get(0..1).ok_or(SaltlickError::InvalidKeyFormat)?;
        let _ = Curve25519Algorithm::from_asn1(algorithm_block)?;

        let (nbits, bitstring) = match key_seq.get(1) {
            Some(ASN1Block::BitString(_, nbits, bitstring)) => (nbits, bitstring),
            _ => return Err(SaltlickError::InvalidKeyFormat),
        };

        if *nbits == PUBLICKEYBYTES * 8 {
            Ok((Self::from_raw_curve25519(&bitstring[..])?, v))
        } else {
            Err(SaltlickError::InvalidKeyFormat)
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq)]
struct Curve25519Algorithm;

impl ToASN1 for Curve25519Algorithm {
    type Error = SaltlickError;

    fn to_asn1_class(&self, _: ASN1Class) -> Result<Vec<ASN1Block>, Self::Error> {
        let oid = ASN1Block::ObjectIdentifier(0, CURVE25519_OID.clone());
        let seq = ASN1Block::Sequence(0, vec![oid]);
        Ok(vec![seq])
    }
}

impl FromASN1 for Curve25519Algorithm {
    type Error = SaltlickError;

    fn from_asn1(v: &[ASN1Block]) -> Result<(Self, &[ASN1Block]), Self::Error> {
        let alg_seq = match v.get(0) {
            Some(ASN1Block::Sequence(_, alg_seq)) => alg_seq,
            _ => return Err(SaltlickError::InvalidKeyFormat),
        };

        let oid = match alg_seq.get(0) {
            Some(ASN1Block::ObjectIdentifier(_, oid)) => oid,
            _ => return Err(SaltlickError::InvalidKeyFormat),
        };

        if oid == *CURVE25519_OID {
            Ok((Curve25519Algorithm, v))
        } else {
            Err(SaltlickError::UnsupportedKeyAlgorithm)
        }
    }
}

/// Wrapper over libsodium-provided secret key type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecretKey {
    pub(crate) inner: SodiumSecretKey,
}

impl SecretKey {
    /// Load secret key from raw Curve25519 bytes.
    ///
    /// This must be 32 bytes long. It corresponds to the key type used by
    /// libsodium 1.x.
    pub fn from_raw_curve25519(bytes: &[u8]) -> Result<SecretKey, SaltlickError> {
        let sodium_key =
            SodiumSecretKey::from_slice(bytes).ok_or(SaltlickError::IncorrectKeyLength)?;
        Ok(SecretKey { inner: sodium_key })
    }

    /// Load secret key from PEM file.
    pub fn from_pem(pem: &str) -> Result<SecretKey, SaltlickError> {
        let pem::Pem { contents, .. } = pem::parse(pem)?;
        simple_asn1::der_decode::<Self>(&contents[..])
    }

    /// Export secret key as PEM-encoded string.
    pub fn to_pem(&self) -> String {
        let der = simple_asn1::der_encode(self).expect("DER-encoding of SecretKey cannot fail");
        pem::encode(&pem::Pem {
            tag: String::from("PRIVATE KEY"),
            contents: der,
        })
    }
}

impl ToASN1 for SecretKey {
    type Error = SaltlickError;

    fn to_asn1_class(&self, _: ASN1Class) -> Result<Vec<ASN1Block>, Self::Error> {
        let mut private_key_asn1 = vec![];
        private_key_asn1.push(ASN1Block::Integer(0, BigInt::from(0u8)));
        private_key_asn1.extend(Curve25519Algorithm.to_asn1()?);
        let wrapped_key =
            simple_asn1::to_der(&ASN1Block::OctetString(0, Vec::from(&self.inner[..])))?;
        private_key_asn1.push(ASN1Block::OctetString(0, wrapped_key));
        Ok(vec![ASN1Block::Sequence(0, private_key_asn1)])
    }
}

impl FromASN1 for SecretKey {
    type Error = SaltlickError;

    fn from_asn1(v: &[ASN1Block]) -> Result<(Self, &[ASN1Block]), Self::Error> {
        let key_seq = match v.get(0) {
            Some(ASN1Block::Sequence(_, key_seq)) => key_seq,
            _ => return Err(SaltlickError::InvalidKeyFormat),
        };

        // Checks that the version is 0
        match key_seq.get(0) {
            Some(ASN1Block::Integer(_, big_int)) => {
                if *big_int != BigInt::from(0u8) {
                    return Err(SaltlickError::UnsupportedVersion);
                }
            }
            _ => return Err(SaltlickError::InvalidKeyFormat),
        }

        // Only checks that algorithm is Curve25519, doesn't actually return the value.
        let algorithm_block = key_seq.get(1..2).ok_or(SaltlickError::InvalidKeyFormat)?;
        let _ = Curve25519Algorithm::from_asn1(algorithm_block)?;

        // The private key uses ASN.1 extensions, and is represented as an
        // octet string encoded within an octet string.
        let secret_key_inner_der = match key_seq.get(2) {
            Some(ASN1Block::OctetString(_, inner_der)) => inner_der,
            _ => return Err(SaltlickError::InvalidKeyFormat),
        };
        let secret_key_block = simple_asn1::from_der(&secret_key_inner_der[..])?;
        let secret_key = match secret_key_block.get(0) {
            Some(ASN1Block::OctetString(_, secret_key)) => secret_key,
            _ => return Err(SaltlickError::InvalidKeyFormat),
        };
        let sodium_secret_key =
            SodiumSecretKey::from_slice(&secret_key[..]).ok_or(SaltlickError::InvalidKeyFormat)?;

        Ok((
            SecretKey {
                inner: sodium_secret_key,
            },
            v,
        ))
    }
}

/// Create a new saltlick keypair.
pub fn gen_keypair() -> (PublicKey, SecretKey) {
    let (raw_public, raw_secret) = box_::gen_keypair();
    (
        PublicKey { inner: raw_public },
        SecretKey { inner: raw_secret },
    )
}

#[cfg(test)]
mod tests {
    use rand::{RngCore, SeedableRng};
    use rand_xorshift::XorShiftRng;

    use super::{PublicKey, SecretKey, PUBLICKEYBYTES};

    const SECRET_KEY: &str = "-----BEGIN PRIVATE KEY-----
    MC4CAQAwBQYDK2VuBCIEIPi/trPNMJy8wbQtVl4oVR60m+7dFksCMU1CJHxQGtxo
    -----END PRIVATE KEY-----";

    const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
    MCowBQYDK2VuAyEA/ZMwRuIZi7mirc8Wx6pet++gYg1wh+7iVT5y2bR0TmU=
    -----END PUBLIC KEY-----";

    fn random_bytes(seed: u64, size: usize) -> Box<[u8]> {
        let mut rng = XorShiftRng::seed_from_u64(seed);
        let mut bytes = vec![0u8; size];
        rng.fill_bytes(&mut bytes);
        bytes.into_boxed_slice()
    }

    #[test]
    fn public_key_from_raw_test() {
        // Any PUBLICKEYBYTES long value is a valid Curve25519 key
        for seed in 1..100 {
            let bytes = random_bytes(seed, PUBLICKEYBYTES);
            assert!(PublicKey::from_raw_curve25519(&bytes[..]).is_ok());
        }
    }

    #[test]
    fn invalid_public_key_from_raw_test() {
        let bytes = random_bytes(0, PUBLICKEYBYTES - 1);
        assert!(PublicKey::from_raw_curve25519(&bytes[..]).is_err());

        let bytes = random_bytes(0, PUBLICKEYBYTES + 1);
        assert!(PublicKey::from_raw_curve25519(&bytes[..]).is_err());

        let bytes = random_bytes(0, 0);
        assert!(PublicKey::from_raw_curve25519(&bytes[..]).is_err());
    }

    #[test]
    fn public_key_pem_round_trip_test() {
        let expected_pem = pem::parse(PUBLIC_KEY).unwrap();
        let public_key = PublicKey::from_pem(PUBLIC_KEY).unwrap();
        let actual_pem = pem::parse(public_key.to_pem()).unwrap();
        assert_eq!(expected_pem, actual_pem);
    }

    #[test]
    fn secret_key_pem_round_trip_test() {
        let expected_pem = pem::parse(SECRET_KEY).unwrap();
        let secret_key = SecretKey::from_pem(SECRET_KEY).unwrap();
        let actual_pem = pem::parse(secret_key.to_pem()).unwrap();
        assert_eq!(expected_pem, actual_pem);
    }
}
