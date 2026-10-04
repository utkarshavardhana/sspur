//! Seeded random distributions; `std_rt.c` section `rng2` mirrors this operation for operation.

pub const GOLDEN: u64 = 0x9E37_79B9_7F4A_7C15;

pub fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn mix_gamma(mut z: u64) -> u64 {
    z = (z ^ (z >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    z = (z ^ (z >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    z = (z ^ (z >> 33)) | 1;
    if (z ^ (z >> 1)).count_ones() < 24 { z ^ 0xAAAA_AAAA_AAAA_AAAA } else { z }
}

#[derive(Clone, Copy)]
pub struct Rng {
    pub s: u64,
    pub g: u64,
}

pub type R<T> = Result<T, &'static str>;

impl Rng {
    pub fn seed(s: i64) -> Rng {
        Rng { s: s as u64, g: GOLDEN }
    }

    pub fn next_u64(&mut self) -> u64 {
        self.s = self.s.wrapping_add(self.g);
        mix(self.s)
    }

    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    pub fn int(&mut self, lo: i64, hi: i64) -> R<i64> {
        if lo >= hi {
            return Err("rand_int needs lo < hi");
        }
        let m = self.next_u64();
        let span = (hi as i128 - lo as i128) as u128;
        Ok((lo as i128 + ((m as u128 * span) >> 64) as i128) as i64)
    }

    pub fn normal(&mut self, mean: f64, sd: f64) -> f64 {
        let u1 = self.f64();
        let u2 = self.f64();
        let r = (-2.0 * (1.0 - u1).ln()).sqrt();
        let c = (std::f64::consts::TAU * u2).cos();
        let z = r * c;
        let t = sd * z;
        mean + t
    }

    pub fn split(&mut self) -> Rng {
        let s = mix(self.next_u64());
        let g = mix_gamma(self.next_u64());
        Rng { s, g }
    }

    pub fn stream(self, k: i64) -> Rng {
        let g = mix_gamma((k as u64).wrapping_mul(GOLDEN).wrapping_add(self.s));
        Rng { s: mix(self.s.wrapping_add(g)), g }
    }

    fn gamma1(&mut self, k: f64) -> f64 {
        if k < 1.0 {
            let g = self.gamma1(k + 1.0);
            let u = self.f64();
            return g * u.powf(1.0 / k);
        }
        let d = k - 1.0 / 3.0;
        let c = 1.0 / (9.0 * d).sqrt();
        loop {
            let (mut x, mut v);
            loop {
                x = self.normal(0.0, 1.0);
                let cx = c * x;
                v = 1.0 + cx;
                if v > 0.0 {
                    break;
                }
            }
            v = v * v * v;
            let u = self.f64();
            let x2 = x * x;
            let x4 = x2 * x2;
            let t = 0.0331 * x4;
            if u < 1.0 - t {
                return d * v;
            }
            let h = 0.5 * x2;
            let w = 1.0 - v + v.ln();
            let dw = d * w;
            if u.ln() < h + dw {
                return d * v;
            }
        }
    }

    pub fn gamma(&mut self, shape: f64, scale: f64) -> R<f64> {
        if !(shape > 0.0 && shape.is_finite() && scale > 0.0 && scale.is_finite()) {
            return Err("rand_gamma needs a finite shape > 0 and scale > 0");
        }
        Ok(self.gamma1(shape) * scale)
    }

    pub fn beta(&mut self, a: f64, b: f64) -> R<f64> {
        if !(a > 0.0 && a.is_finite() && b > 0.0 && b.is_finite()) {
            return Err("rand_beta needs finite a > 0 and b > 0");
        }
        Ok(self.beta1(a, b))
    }

    fn beta1(&mut self, a: f64, b: f64) -> f64 {
        let x = self.gamma1(a);
        let y = self.gamma1(b);
        x / (x + y)
    }

    pub fn binomial(&mut self, n: i64, p: f64) -> R<i64> {
        if n < 0 || !(0.0..=1.0).contains(&p) {
            return Err("rand_binomial needs n >= 0 and 0 <= p <= 1");
        }
        Ok(self.binomial1(n, p))
    }

    fn binomial1(&mut self, mut n: i64, mut p: f64) -> i64 {
        let mut k = 0i64;
        while n > 16 {
            if p <= 0.0 {
                return k;
            }
            if p >= 1.0 {
                return k + n;
            }
            let a = 1 + n / 2;
            let b = n - a + 1;
            let x = self.beta1(a as f64, b as f64);
            if x >= p {
                n = a - 1;
                p /= x;
            } else {
                k += a;
                n = b - 1;
                p = (p - x) / (1.0 - x);
            }
        }
        for _ in 0..n {
            if self.f64() < p {
                k += 1;
            }
        }
        k
    }

    pub fn poisson(&mut self, mean: f64) -> R<i64> {
        if !(0.0..=4.0e15).contains(&mean) {
            return Err("rand_poisson needs 0 <= mean <= 4e15");
        }
        let mut mu = mean;
        let mut k = 0i64;
        while mu > 16.0 {
            let m = (mu * 0.875).floor() as i64;
            let x = self.gamma1(m as f64);
            if x >= mu {
                return Ok(k + self.binomial1(m - 1, mu / x));
            }
            k += m;
            mu -= x;
        }
        let l = (-mu).exp();
        let mut q = self.f64();
        while q > l {
            k += 1;
            q *= self.f64();
        }
        Ok(k)
    }

    pub fn geometric(&mut self, p: f64) -> R<i64> {
        if !(p > 0.0 && p <= 1.0) {
            return Err("rand_geometric needs 0 < p <= 1");
        }
        let u = self.f64();
        if p == 1.0 {
            return Ok(0);
        }
        let x = ((-u).ln_1p() / (-p).ln_1p()).floor();
        if x >= 9.223_372_036_854_776e18 {
            return Err("integer overflow");
        }
        Ok(x as i64)
    }

    pub fn weighted(&mut self, ws: &[f64]) -> R<i64> {
        let mut total = 0.0;
        for &w in ws {
            if !(w >= 0.0 && w.is_finite()) {
                return Err("rand_weighted needs finite weights >= 0 with a positive sum");
            }
            total += w;
        }
        if !(total > 0.0 && total.is_finite()) {
            return Err("rand_weighted needs finite weights >= 0 with a positive sum");
        }
        let u = self.f64() * total;
        let mut c = 0.0;
        let mut last = 0;
        for (i, &w) in ws.iter().enumerate() {
            if w > 0.0 {
                last = i;
            }
            c += w;
            if u < c {
                return Ok(i as i64);
            }
        }
        Ok(last as i64)
    }
}
