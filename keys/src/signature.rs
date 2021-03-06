use std::fmt;
use std::str::FromStr;

use byteorder::{ByteOrder, LittleEndian};
use secp256k1::recovery::{RecoverableSignature, RecoveryId};

use crate::{base58, error, hash};

/// An secp256k1 signature.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Signature(RecoverableSignature);

impl Signature {
    pub fn is_canonical(&self) -> bool {
        self.0.is_canonical()
    }

    pub fn to_standard(&self) -> secp256k1::Signature {
        self.0.to_standard()
    }

    pub fn serialize_compact(&self) -> [u8; 65] {
        let (recovery_id, sig) = self.0.serialize_compact();
        let mut data: [u8; 65] = [0u8; 65];
        data[0] = recovery_id.to_i32() as u8 + 27 + 4;
        data[1..65].copy_from_slice(&sig[..]);
        data
    }

    pub fn from_compact(data: &[u8; 65]) -> crate::Result<Self> {
        let id = if data[0] >= 31 {
            (data[0] - 4 - 27) as i32
        } else {
            data[0] as i32
        };
        let recv_id = RecoveryId::from_i32(id)?;
        let recv_sig = RecoverableSignature::from_compact(&data[1..], recv_id)?;
        Ok(Self(recv_sig))
    }
}

impl From<RecoverableSignature> for Signature {
    fn from(recv_sig: RecoverableSignature) -> Signature {
        Signature(recv_sig)
    }
}

impl FromStr for Signature {
    type Err = error::Error;

    fn from_str(s: &str) -> crate::Result<Signature> {
        if !s.starts_with("SIG_K1_") {
            return Err(secp256k1::Error::InvalidSignature.into());
        }

        let s_hex = base58::from(&s[7..])?;
        // recovery id length: 1
        // signature length: 64
        // checksum length: 4
        if s_hex.len() != 1 + 64 + 4 {
            return Err(secp256k1::Error::InvalidSignature.into());
        }

        let recid = secp256k1::recovery::RecoveryId::from_i32((s_hex[0] - 4 - 27) as i32)?;
        let data = &s_hex[1..65];

        // Verify checksum
        let mut checksum_data = [0u8; 67];
        checksum_data[..65].copy_from_slice(&s_hex[..65]);
        checksum_data[65..67].copy_from_slice(b"K1");
        let expected = LittleEndian::read_u32(&hash::ripemd160(&checksum_data)[..4]);
        let actual = LittleEndian::read_u32(&s_hex[65..69]);
        if expected != actual {
            return Err(base58::Error::BadChecksum(expected, actual).into());
        }

        let rec_sig = secp256k1::recovery::RecoverableSignature::from_compact(&data, recid)?;

        Ok(Signature(rec_sig))
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (recovery_id, sig) = self.0.serialize_compact();

        // See https://github.com/EOSIO/fc/blob/f4755d330faf9d2342d646a93f9a27bf68ca759e/src/crypto/elliptic_impl_priv.cpp
        let mut checksum_data: [u8; 67] = [0u8; 67];
        checksum_data[0] = recovery_id.to_i32() as u8 + 27 + 4;
        checksum_data[1..65].copy_from_slice(&sig[..]);
        checksum_data[65..67].copy_from_slice(b"K1");

        // Compute ripemd160 checksum
        let checksum_h160 = hash::ripemd160(&checksum_data);
        let checksum = &checksum_h160.take()[..4];

        // Signature slice
        let mut sig_slice: [u8; 69] = [0u8; 69];
        sig_slice[..65].copy_from_slice(&checksum_data[..65]);
        sig_slice[65..69].copy_from_slice(&checksum[..]);

        write!(f, "SIG_K1_{}", base58::encode_slice(&sig_slice))?;

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::Signature;

    #[test]
    fn sig_from_str_should_work() {
        let sig_str = "SIG_K1_KBJgSuRYtHZcrWThugi4ygFabto756zuQQo8XeEpyRtBXLb9kbJtNW3xDcS14Rc14E8iHqLrdx46nenG5T7R4426Bspyzk";
        let sig = Signature::from_str(sig_str);
        assert!(sig.is_ok());
        assert!(sig.unwrap().is_canonical());
    }

    #[test]
    fn sig_from_str_should_error() {
        let sig_str = "KomV6FEHKdtZxGDwhwSubEAcJ7VhtUQpEt5P6iDz33ic936aSXx87B2L56C8JLQkqNpp1W8ZXjrKiLHUEB4LCGeXvbtVuR";
        let sig = Signature::from_str(sig_str);
        assert!(sig.is_err());
    }
}
