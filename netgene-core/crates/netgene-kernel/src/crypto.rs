use num_bigint::{BigInt, RandBigInt};
use num_traits::{One, Zero};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub n: BigInt,
    pub n_sq: BigInt,
    pub g: BigInt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateKey {
    pub lambda: BigInt,
    pub mu: BigInt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keypair {
    pub public: PublicKey,
    pub private: PrivateKey,
}

impl Keypair {
    /// Generate a weak Paillier keypair for demonstration (do NOT use in production without 2048-bit primes).
    pub fn generate() -> Self {
        let _rng = rand::thread_rng();
        // Use very small primes for fast demo compilation and execution (e.g., 64-bit primes)
        // Note: For real FHE, we would use proper prime generation. 
        // Here we simulate it with hardcoded safe pseudo-primes or small random numbers for instantaneous UI demo.
        let p = BigInt::from(1000000007u64);
        let q = BigInt::from(1000000009u64);
        
        let n = &p * &q;
        let n_sq = &n * &n;
        let g = &n + BigInt::one();

        let p_minus_1 = &p - BigInt::one();
        let q_minus_1 = &q - BigInt::one();
        let lambda = lcm(&p_minus_1, &q_minus_1);

        let l_val = (g.modpow(&lambda, &n_sq) - BigInt::one()) / &n;
        let mu = mod_inverse(&l_val, &n).unwrap_or(BigInt::one());

        Self {
            public: PublicKey { n, n_sq, g },
            private: PrivateKey { lambda, mu },
        }
    }
}

/// Helper function: Least Common Multiple
fn lcm(a: &BigInt, b: &BigInt) -> BigInt {
    (a * b) / gcd(a.clone(), b.clone())
}

/// Helper function: Greatest Common Divisor
fn gcd(mut a: BigInt, mut b: BigInt) -> BigInt {
    while !b.is_zero() {
        let t = b.clone();
        b = a % b;
        a = t;
    }
    a
}

/// Helper function: Modular Inverse using Extended Euclidean Algorithm
fn mod_inverse(a: &BigInt, n: &BigInt) -> Option<BigInt> {
    let mut t = BigInt::zero();
    let mut newt = BigInt::one();
    let mut r = n.clone();
    let mut newr = a.clone();

    while !newr.is_zero() {
        let quotient = &r / &newr;
        
        let temp_t = t.clone();
        t = newt.clone();
        newt = temp_t - &quotient * &newt;
        
        let temp_r = r.clone();
        r = newr.clone();
        newr = temp_r - &quotient * &newr;
    }

    if r > BigInt::one() {
        return None;
    }
    if t < BigInt::zero() {
        t = t + n;
    }
    Some(t)
}

pub struct PaillierFHE;

impl PaillierFHE {
    /// Encrypt a message `m` using the public key
    pub fn encrypt(pub_key: &PublicKey, m: u64) -> BigInt {
        let mut rng = rand::thread_rng();
        let r = rng.gen_bigint_range(&BigInt::one(), &pub_key.n);
        
        let m_big = BigInt::from(m);
        let gm = pub_key.g.modpow(&m_big, &pub_key.n_sq);
        let rn = r.modpow(&pub_key.n, &pub_key.n_sq);
        
        (gm * rn) % &pub_key.n_sq
    }

    /// Decrypt a ciphertext `c` using the private key
    pub fn decrypt(pub_key: &PublicKey, priv_key: &PrivateKey, c: &BigInt) -> u64 {
        let u = c.modpow(&priv_key.lambda, &pub_key.n_sq);
        let l_val = (u - BigInt::one()) / &pub_key.n;
        let m = (l_val * &priv_key.mu) % &pub_key.n;
        m.try_into().unwrap_or(0)
    }

    /// Homomorphically add two encrypted values: E(m1) ⊕ E(m2) = E(m1 + m2)
    pub fn add_encrypted(pub_key: &PublicKey, c1: &BigInt, c2: &BigInt) -> BigInt {
        (c1 * c2) % &pub_key.n_sq
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paillier_homomorphic_addition() {
        let kp = Keypair::generate();
        let c1 = PaillierFHE::encrypt(&kp.public, 15);
        let c2 = PaillierFHE::encrypt(&kp.public, 25);
        
        // Kernel performs this without knowing 15 or 25
        let c_sum = PaillierFHE::add_encrypted(&kp.public, &c1, &c2);
        
        // Node decrypts the result
        let result = PaillierFHE::decrypt(&kp.public, &kp.private, &c_sum);
        assert_eq!(result, 40);
    }
}
