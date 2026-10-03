use num_bigint::BigUint;
use num_traits::One;
use rand_core::{CryptoRng, RngCore};
use sha3::{Digest, Keccak256};

#[derive(Clone, Debug)]
pub struct VDFParams {
    pub n: BigUint,
    pub t: u64,
}

impl VDFParams {
    pub fn new(n: BigUint, t: u64) -> Self {
        Self { n, t }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VDFProof {
    pub y: String,
    pub x: String,
    pub t: u64,
    pub n: String,
    pub halfway_points: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct VDFResult {
    pub y: BigUint,
    pub proof: Vec<BigUint>,
}

/// Left-pad a big integer into exactly 32 bytes (big-endian), matching the
/// Solidity `abi.encodePacked(uint256)` representation used on-chain.
fn to_be32(value: &BigUint) -> [u8; 32] {
    let bytes = value.to_bytes_be();
    let mut out = [0u8; 32];
    if bytes.len() <= 32 {
        out[32 - bytes.len()..].copy_from_slice(&bytes);
    } else {
        out.copy_from_slice(&bytes[bytes.len() - 32..]);
    }
    out
}

/// Fiat-Shamir challenge, matching `VDFVerifier.verifyVDF` exactly:
/// `uint256(keccak256(abi.encodePacked(x, y, mu, n))) % n`.
pub fn hash_to_challenge(x: &BigUint, y: &BigUint, mu: &BigUint, n: &BigUint) -> BigUint {
    let mut hasher = Keccak256::new();
    hasher.update(to_be32(x));
    hasher.update(to_be32(y));
    hasher.update(to_be32(mu));
    hasher.update(to_be32(n));
    BigUint::from_bytes_be(&hasher.finalize()) % n
}

pub fn compute_vdf(x: &BigUint, params: &VDFParams) -> BigUint {
    let exp = BigUint::one() << params.t;
    x.modpow(&exp, &params.n)
}

pub fn compute_vdf_with_proof(x: &BigUint, params: &VDFParams) -> VDFResult {
    let y = compute_vdf(x, params);
    let mut proof_mu_list: Vec<BigUint> = Vec::new();
    generate_proof(x, &y, params.t, &params.n, &mut proof_mu_list);
    VDFResult {
        y,
        proof: proof_mu_list,
    }
}

/// Generate a Pietrzak proof. At each round split the remaining exponent into
/// `a = t/2` (floor) and `b = t - a` (ceil), emit the midpoint `mu = x^(2^a)`,
/// then advance the statement exactly like the verifier:
///   r = H(x, y, mu)
///   x = x^r * mu
///   y = mu^(r * 2^(b-a)) * y
///   t = b
/// For even `t` the halves are equal (`b-a = 0`); for odd `t` the verifier
/// doubles the exponent by squaring `mu^r` (`b-a = 1`).
fn generate_proof(x: &BigUint, y: &BigUint, t: u64, n: &BigUint, proof_list: &mut Vec<BigUint>) {
    if t <= 1 {
        return;
    }
    let mut cur_x = x.clone();
    let mut cur_y = y.clone();
    let mut t_current = t;
    while t_current > 1 {
        let a = t_current / 2;
        let b = t_current - a;
        let mu = cur_x.modpow(&(BigUint::one() << a), n);
        proof_list.push(mu.clone());

        let r = hash_to_challenge(&cur_x, &cur_y, &mu, n);
        let x_r = cur_x.modpow(&r, n);
        let mut mu_r = mu.modpow(&r, n);
        if b != a {
            // odd step: fold the extra doubling of the exponent into y
            mu_r = (&mu_r * &mu_r) % n;
        }
        cur_x = (x_r * &mu) % n;
        cur_y = (mu_r * &cur_y) % n;
        t_current = b;
    }
}

pub fn verify_vdf_pietrzak(x: &BigUint, y: &BigUint, proof: &[BigUint], t: u64, n: &BigUint) -> bool {
    if t == 0 {
        return x == y;
    }
    if t == 1 {
        let x2 = (x * x) % n;
        return x2 == *y;
    }
    let mut cur_x = x.clone();
    let mut cur_y = y.clone();
    let mut cur_t = t;
    let mut proof_iter = proof.iter();
    let mut steps = 0;
    while cur_t > 1 && steps < 10000 {
        if let Some(mu) = proof_iter.next() {
            let a = cur_t / 2;
            let b = cur_t - a;
            let r = hash_to_challenge(&cur_x, &cur_y, mu, n);
            let cur_x_r = cur_x.modpow(&r, n);
            let mut mu_r = mu.modpow(&r, n);
            if b != a {
                mu_r = (&mu_r * &mu_r) % n;
            }
            cur_x = (&cur_x_r * mu) % n;
            cur_y = (&mu_r * &cur_y) % n;
            cur_t = b;
        } else {
            let exp = BigUint::one() << cur_t;
            let computed = cur_x.modpow(&exp, n);
            return computed == cur_y;
        }
        steps += 1;
    }
    if cur_t == 1 {
        let x2 = (&cur_x * &cur_x) % n;
        return x2 == cur_y;
    }
    let exp = BigUint::one() << cur_t;
    let computed = cur_x.modpow(&exp, n);
    computed == cur_y
}

pub fn generate_rsa_modulus<R: RngCore + CryptoRng>(_rng: &mut R, _bits: usize) -> BigUint {
    BigUint::from(0x10001u64) * BigUint::from(0x7fffffff12345678u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n() -> BigUint {
        BigUint::from(0x10001u64) * BigUint::from(0x7fffffff12345678u64)
    }

    #[test]
    fn challenge_matches_solidity_abi_encode_packed_keccak() {
        // Values pinned from the on-chain Solidity verifier encoding:
        // keccak256(abi.encodePacked(x, y, mu, n)) with 32-byte big-endian words.
        let x = BigUint::parse_bytes(
            b"98259812377084674167903666581508149273936636147602228205984045089377405103448",
            10,
        )
        .unwrap();
        let y = BigUint::parse_bytes(b"303572522159900148211096", 10).unwrap();
        let mu = BigUint::parse_bytes(b"76d003d0dfa42612d38", 16).unwrap();
        let n = n();

        let packed_hash = BigUint::parse_bytes(
            b"282e7c56d85d26002700bdefa5f6da287966e184c0966ce59e1ae492b533ec4c",
            16,
        )
        .unwrap();
        let expected = &packed_hash % &n;

        assert_eq!(hash_to_challenge(&x, &y, &mu, &n), expected);
    }

    #[test]
    fn proof_round_trips() {
        let n = n();
        for t in [1u64, 2, 3, 5, 8, 17, 100] {
            let x = BigUint::from(0xdeadbeefu64) + t;
            let params = VDFParams::new(n.clone(), t);
            let result = compute_vdf_with_proof(&x, &params);
            assert!(
                verify_vdf_pietrzak(&x, &result.y, &result.proof, t, &n),
                "proof failed to verify for t={t}"
            );
        }
    }
}

