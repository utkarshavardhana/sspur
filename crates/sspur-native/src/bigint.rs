use std::cmp::Ordering;

pub const MAX_LIMBS: usize = 1 << 22;
pub const MAX_SCALE: i64 = 10_000;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Big {
    pub neg: bool,
    pub mag: Vec<u32>,
}

fn trim(mut v: Vec<u32>) -> Vec<u32> {
    while v.last() == Some(&0) {
        v.pop();
    }
    v
}

pub fn mag_cmp(a: &[u32], b: &[u32]) -> Ordering {
    a.len().cmp(&b.len()).then_with(|| a.iter().rev().cmp(b.iter().rev()))
}

fn mag_add(a: &[u32], b: &[u32]) -> Vec<u32> {
    let (a, b) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut out = Vec::with_capacity(a.len() + 1);
    let mut carry = 0u64;
    for i in 0..a.len() {
        let t = u64::from(a[i]) + u64::from(*b.get(i).unwrap_or(&0)) + carry;
        out.push(t as u32);
        carry = t >> 32;
    }
    if carry > 0 {
        out.push(carry as u32);
    }
    out
}

fn mag_sub(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0i64;
    for i in 0..a.len() {
        let mut t = i64::from(a[i]) - i64::from(*b.get(i).unwrap_or(&0)) - borrow;
        borrow = 0;
        if t < 0 {
            t += 1 << 32;
            borrow = 1;
        }
        out.push(t as u32);
    }
    trim(out)
}

fn mag_mul(a: &[u32], b: &[u32]) -> Vec<u32> {
    if a.is_empty() || b.is_empty() {
        return vec![];
    }
    let mut out = vec![0u32; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0u64;
        for (j, &y) in b.iter().enumerate() {
            let t = u64::from(x) * u64::from(y) + u64::from(out[i + j]) + carry;
            out[i + j] = t as u32;
            carry = t >> 32;
        }
        out[i + b.len()] = carry as u32;
    }
    trim(out)
}

fn divrem_small(a: &[u32], d: u32) -> (Vec<u32>, u32) {
    let mut q = vec![0u32; a.len()];
    let mut r = 0u64;
    for i in (0..a.len()).rev() {
        let cur = (r << 32) | u64::from(a[i]);
        q[i] = (cur / u64::from(d)) as u32;
        r = cur % u64::from(d);
    }
    (trim(q), r as u32)
}

fn shl_bits(v: &[u32], s: u32, extra: bool) -> Vec<u32> {
    let mut out = Vec::with_capacity(v.len() + 1);
    let mut prev = 0u32;
    for &x in v {
        out.push(if s == 0 { x } else { (x << s) | (prev >> (32 - s)) });
        prev = x;
    }
    if extra {
        out.push(if s == 0 { 0 } else { prev >> (32 - s) });
    }
    out
}

pub fn mag_divmod(u: &[u32], v: &[u32]) -> (Vec<u32>, Vec<u32>) {
    if mag_cmp(u, v) == Ordering::Less {
        return (vec![], u.to_vec());
    }
    if v.len() == 1 {
        let (q, r) = divrem_small(u, v[0]);
        return (q, if r == 0 { vec![] } else { vec![r] });
    }
    let n = v.len();
    let m = u.len() - n;
    let s = v[n - 1].leading_zeros();
    let vn = shl_bits(v, s, false);
    let mut un = shl_bits(u, s, true);
    let mut q = vec![0u32; m + 1];
    let b = 1u64 << 32;
    for j in (0..=m).rev() {
        let num = (u64::from(un[j + n]) << 32) | u64::from(un[j + n - 1]);
        let mut qhat = num / u64::from(vn[n - 1]);
        let mut rhat = num - qhat * u64::from(vn[n - 1]);
        while qhat >= b || qhat * u64::from(vn[n - 2]) > b * rhat + u64::from(un[j + n - 2]) {
            qhat -= 1;
            rhat += u64::from(vn[n - 1]);
            if rhat >= b {
                break;
            }
        }
        let mut k = 0i64;
        for i in 0..n {
            let p = qhat * u64::from(vn[i]);
            let t = i64::from(un[i + j]) - k - (p & 0xFFFF_FFFF) as i64;
            un[i + j] = t as u32;
            k = (p >> 32) as i64 - (t >> 32);
        }
        let t = i64::from(un[j + n]) - k;
        un[j + n] = t as u32;
        q[j] = qhat as u32;
        if t < 0 {
            q[j] = q[j].wrapping_sub(1);
            let mut c = 0u64;
            for i in 0..n {
                let t = u64::from(un[i + j]) + u64::from(vn[i]) + c;
                un[i + j] = t as u32;
                c = t >> 32;
            }
            un[j + n] = un[j + n].wrapping_add(c as u32);
        }
    }
    let r: Vec<u32> = (0..n).map(|i| if s == 0 { un[i] } else { (un[i] >> s) | (un[i + 1] << (32 - s)) }).collect();
    (trim(q), trim(r))
}

fn norm(neg: bool, mag: Vec<u32>) -> Big {
    let mag = trim(mag);
    Big { neg: neg && !mag.is_empty(), mag }
}

pub enum BigErr {
    DivZero,
    Oom,
    NegExp,
}

impl Big {
    pub fn from_i64(n: i64) -> Big {
        let u = n.unsigned_abs();
        norm(n < 0, vec![u as u32, (u >> 32) as u32])
    }

    pub fn to_i64(&self) -> Option<i64> {
        if self.mag.len() > 2 {
            return None;
        }
        let u = u64::from(*self.mag.first().unwrap_or(&0)) | (u64::from(*self.mag.get(1).unwrap_or(&0)) << 32);
        if self.neg {
            if u <= 1 << 63 { Some((u as i64).wrapping_neg()) } else { None }
        } else {
            i64::try_from(u).ok()
        }
    }

    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    pub fn neg(&self) -> Big {
        norm(!self.neg, self.mag.clone())
    }

    pub fn abs(&self) -> Big {
        norm(false, self.mag.clone())
    }

    pub fn sign(&self) -> i64 {
        if self.mag.is_empty() { 0 } else if self.neg { -1 } else { 1 }
    }

    pub fn add(&self, o: &Big) -> Big {
        if self.neg == o.neg {
            return norm(self.neg, mag_add(&self.mag, &o.mag));
        }
        match mag_cmp(&self.mag, &o.mag) {
            Ordering::Less => norm(o.neg, mag_sub(&o.mag, &self.mag)),
            _ => norm(self.neg, mag_sub(&self.mag, &o.mag)),
        }
    }

    pub fn sub(&self, o: &Big) -> Big {
        self.add(&o.neg())
    }

    pub fn mul(&self, o: &Big) -> Result<Big, BigErr> {
        if self.mag.len() + o.mag.len() > MAX_LIMBS {
            return Err(BigErr::Oom);
        }
        Ok(norm(self.neg != o.neg, mag_mul(&self.mag, &o.mag)))
    }

    pub fn divmod(&self, o: &Big) -> Result<(Big, Big), BigErr> {
        if o.is_zero() {
            return Err(BigErr::DivZero);
        }
        let (q, r) = mag_divmod(&self.mag, &o.mag);
        Ok((norm(self.neg != o.neg, q), norm(self.neg, r)))
    }

    fn bits(&self) -> u64 {
        match self.mag.last() {
            None => 0,
            Some(top) => 32 * (self.mag.len() as u64 - 1) + u64::from(32 - top.leading_zeros()),
        }
    }

    pub fn pow(&self, e: i64) -> Result<Big, BigErr> {
        if e < 0 {
            return Err(BigErr::NegExp);
        }
        if e == 0 {
            return Ok(Big::from_i64(1));
        }
        if self.mag.len() == 1 && self.mag[0] == 1 || self.is_zero() {
            return Ok(norm(self.neg && e % 2 == 1, self.mag.clone()));
        }
        if u128::from(self.bits()) * e as u128 > (MAX_LIMBS as u128) * 32 {
            return Err(BigErr::Oom);
        }
        let mut result = Big::from_i64(1);
        let mut base = self.clone();
        let mut e = e as u64;
        while e > 0 {
            if e & 1 == 1 {
                result = result.mul(&base)?;
            }
            e >>= 1;
            if e > 0 {
                base = base.mul(&base)?;
            }
        }
        Ok(result)
    }

    pub fn cmp(&self, o: &Big) -> Ordering {
        match (self.neg, o.neg) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => mag_cmp(&self.mag, &o.mag),
            (true, true) => mag_cmp(&o.mag, &self.mag),
        }
    }

    pub fn parse(s: &str) -> Option<Big> {
        let t = s.trim();
        let (neg, digits) = match t.as_bytes().first() {
            Some(b'-') => (true, &t[1..]),
            Some(b'+') => (false, &t[1..]),
            _ => (false, t),
        };
        if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(from_digits(neg, digits))
    }

    pub fn pow10(k: i64) -> Big {
        let mut r = Big::from_i64(1);
        let mut k = k;
        while k > 0 {
            let step = k.min(9);
            r = norm(false, mag_mul(&r.mag, &[10u32.pow(step as u32)]));
            k -= step;
        }
        r
    }
}

fn from_digits(neg: bool, digits: &str) -> Big {
    let mut mag: Vec<u32> = vec![];
    let b = digits.as_bytes();
    let first = b.len() % 9;
    let mut i = 0;
    while i < b.len() {
        let w = if i == 0 && first != 0 { first } else { 9 };
        let chunk = b[i..i + w].iter().fold(0u32, |a, c| a * 10 + u32::from(c - b'0'));
        mag = mag_mul(&mag, &[10u32.pow(w as u32)]);
        mag = mag_add(&mag, &[chunk]);
        i += w;
    }
    norm(neg, mag)
}

impl std::fmt::Display for Big {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.mag.is_empty() {
            return write!(f, "0");
        }
        let mut parts = Vec::new();
        let mut cur = self.mag.clone();
        while !cur.is_empty() {
            let (q, r) = divrem_small(&cur, 1_000_000_000);
            parts.push(r);
            cur = q;
        }
        let mut s = String::new();
        if self.neg {
            s.push('-');
        }
        s += &parts.last().unwrap().to_string();
        for p in parts.iter().rev().skip(1) {
            s += &format!("{p:09}");
        }
        write!(f, "{s}")
    }
}

pub fn dec_str(m: &Big, scale: i64) -> String {
    let digits = m.abs().to_string();
    let s = scale as usize;
    let body = if s == 0 {
        digits
    } else if digits.len() > s {
        format!("{}.{}", &digits[..digits.len() - s], &digits[digits.len() - s..])
    } else {
        format!("0.{}{digits}", "0".repeat(s - digits.len()))
    };
    if m.neg { format!("-{body}") } else { body }
}

pub fn dec_parse(s: &str) -> Option<(Big, i64)> {
    let t = s.trim();
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let (int, frac) = rest.split_once('.').unwrap_or((rest, ""));
    if int.is_empty() || !int.bytes().all(|c| c.is_ascii_digit()) || !frac.bytes().all(|c| c.is_ascii_digit()) || (rest.contains('.') && frac.is_empty()) || frac.len() as i64 > MAX_SCALE {
        return None;
    }
    Some((from_digits(neg, &format!("{int}{frac}")), frac.len() as i64))
}

pub fn dec_cmp(a: &Big, sa: i64, b: &Big, sb: i64) -> Ordering {
    let s = sa.max(sb);
    let x = if s > sa { norm(a.neg, mag_mul(&a.mag, &Big::pow10(s - sa).mag)) } else { a.clone() };
    let y = if s > sb { norm(b.neg, mag_mul(&b.mag, &Big::pow10(s - sb).mag)) } else { b.clone() };
    x.cmp(&y).then(sa.cmp(&sb))
}

pub fn rescale(m: &Big, from: i64, to: i64) -> Big {
    if to >= from {
        return norm(m.neg, mag_mul(&m.mag, &Big::pow10(to - from).mag));
    }
    let d = Big::pow10(from - to);
    let (q, r) = mag_divmod(&m.mag, &d.mag);
    round_half_even(m.neg, q, &r, &d.mag)
}

pub fn round_half_even(neg: bool, q: Vec<u32>, r: &[u32], d: &[u32]) -> Big {
    let twice = mag_add(r, r);
    let up = match mag_cmp(&twice, d) {
        Ordering::Greater => true,
        Ordering::Equal => q.first().is_some_and(|x| x & 1 == 1),
        Ordering::Less => false,
    };
    let q = if up { mag_add(&q, &[1]) } else { q };
    norm(neg, q)
}

pub fn dec_div(a: &Big, sa: i64, b: &Big, sb: i64, scale: i64) -> Result<Big, BigErr> {
    if b.is_zero() {
        return Err(BigErr::DivZero);
    }
    let num = mag_mul(&a.mag, &Big::pow10(scale + sb).mag);
    let den = mag_mul(&b.mag, &Big::pow10(sa).mag);
    let (q, r) = mag_divmod(&num, &den);
    Ok(round_half_even(a.neg != b.neg, q, &r, &den))
}
