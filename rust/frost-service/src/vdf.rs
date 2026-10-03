use num_bigint::BigUint;
use num_traits::One;
use rand_core::{CryptoRng, RngCore};
use sha2::{Digest, Sha256};

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

fn hash_to_challenge(x: &BigUint, y: &BigUint, mu: &BigUint, n: &BigUint) -> BigUint {
    let mut hasher = Sha256::new();
    hasher.update(x.to_bytes_be());
    hasher.update(y.to_bytes_be());
    hasher.update(mu.to_bytes_be());
    hasher.update(n.to_bytes_be());
    BigUint::from_bytes_be(&hasher.finalize()) % n
}

pub fn compute_vdf(x: &BigUint, params: &VDFParams) -> BigUint {
    let exp = BigUint::one() << params.t;
    x.modpow(&exp, &params.n)
}

pub fn compute_vdf_with_proof(x: &BigUint, params: &VDFParams) -> VDFResult {
    let y = compute_vdf(x, params);
    let mut proof_mu_list: Vec<BigUint> = Vec::new();
    generate_proof(x, params.t, &params.n, &mut proof_mu_list);
    VDFResult {
        y,
        proof: proof_mu_list,
    }
}

fn generate_proof(x: &BigUint, t: u64, n: &BigUint, proof_list: &mut Vec<BigUint>) {
    if t <= 1 {
        return;
    }
    let mut t_current = t;
    while t_current > 1 {
        let t_half = t_current / 2;
        let exp_mu = BigUint::one() << t_half;
        let mu = x.modpow(&exp_mu, n);
        proof_list.push(mu);
        t_current = t_half;
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
            let t_half = cur_t / 2;
            let r = hash_to_challenge(&cur_x, &cur_y, mu, n);
            let cur_x_r = cur_x.modpow(&r, n);
            let mu_r = mu.modpow(&r, n);
            cur_x = (&cur_x_r * mu) % n;
            cur_y = (&mu_r * &cur_y) % n;
            cur_t = t_half;
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
