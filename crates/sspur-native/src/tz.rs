//! IANA time zones from TZif files with POSIX TZ footers; `std_rt.c` section `tz` mirrors this.
use crate::chrono::{civil, days_from_civil, mdays};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Zone {
    pub name: String,
    pub trans: Vec<i64>,
    pub idx: Vec<i64>,
    pub offs: Vec<i64>,
    pub dst: Vec<bool>,
    pub abbrs: Vec<String>,
    pub rule: Vec<i64>,
}

pub fn fixed(name: &str, secs: i64) -> Zone {
    Zone { name: name.into(), offs: vec![secs], dst: vec![false], abbrs: vec![name.into()], ..Zone::default() }
}

pub fn offset_name(secs: i64) -> String {
    let a = secs.abs();
    format!("{}{:02}:{:02}", if secs < 0 { '-' } else { '+' }, a / 3600, a / 60 % 60)
}

/// `UTC`, `Z`, `+HH`, `+HHMM` or `+HH:MM`, either sign.
pub fn parse_fixed(s: &str) -> Option<i64> {
    if s == "UTC" || s == "Z" {
        return Some(0);
    }
    let b = s.as_bytes();
    if b.len() < 3 || (b[0] != b'+' && b[0] != b'-') {
        return None;
    }
    let d = &b[1..];
    let two = |x: &[u8]| (x[0].is_ascii_digit() && x[1].is_ascii_digit()).then(|| (x[0] - b'0') as i64 * 10 + (x[1] - b'0') as i64);
    let (h, m) = match d.len() {
        2 => (two(d)?, 0),
        4 => (two(&d[0..2])?, two(&d[2..4])?),
        5 if d[2] == b':' => (two(&d[0..2])?, two(&d[3..5])?),
        _ => return None,
    };
    if h > 23 || m > 59 {
        return None;
    }
    let v = h * 3600 + m * 60;
    Some(if b[0] == b'-' { -v } else { v })
}

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

impl P<'_> {
    fn peek(&self) -> u8 {
        self.b.get(self.i).copied().unwrap_or(0)
    }

    fn num(&mut self, max: i64) -> Option<i64> {
        let start = self.i;
        let mut v = 0i64;
        while self.peek().is_ascii_digit() && self.i - start < 4 {
            v = v * 10 + (self.peek() - b'0') as i64;
            self.i += 1;
        }
        if self.i == start || v > max {
            return None;
        }
        Some(v)
    }

    fn name(&mut self) -> Option<String> {
        let start;
        let end;
        if self.peek() == b'<' {
            self.i += 1;
            start = self.i;
            while self.peek() != b'>' {
                if self.peek() == 0 {
                    return None;
                }
                self.i += 1;
            }
            end = self.i;
            self.i += 1;
        } else {
            start = self.i;
            while self.peek().is_ascii_alphabetic() {
                self.i += 1;
            }
            end = self.i;
        }
        if end - start < 3 {
            return None;
        }
        String::from_utf8(self.b[start..end].to_vec()).ok()
    }

    fn hms(&mut self, max_h: i64) -> Option<i64> {
        let neg = match self.peek() {
            b'-' => {
                self.i += 1;
                true
            }
            b'+' => {
                self.i += 1;
                false
            }
            _ => false,
        };
        let mut v = self.num(max_h)? * 3600;
        if self.peek() == b':' {
            self.i += 1;
            v += self.num(59)? * 60;
            if self.peek() == b':' {
                self.i += 1;
                v += self.num(59)?;
            }
        }
        Some(if neg { -v } else { v })
    }

    fn date(&mut self, out: &mut Vec<i64>) -> Option<()> {
        match self.peek() {
            b'M' => {
                self.i += 1;
                let m = self.num(12)?;
                if m < 1 || self.peek() != b'.' {
                    return None;
                }
                self.i += 1;
                let w = self.num(5)?;
                if w < 1 || self.peek() != b'.' {
                    return None;
                }
                self.i += 1;
                let d = self.num(6)?;
                out.extend([0, m, w, d]);
            }
            b'J' => {
                self.i += 1;
                let n = self.num(365)?;
                if n < 1 {
                    return None;
                }
                out.extend([1, n, 0, 0]);
            }
            _ => out.extend([2, self.num(365)?, 0, 0]),
        }
        let t = if self.peek() == b'/' {
            self.i += 1;
            self.hms(167)?
        } else {
            7200
        };
        out.push(t);
        Some(())
    }
}

fn add_type(z: &mut Zone, off: i64, dst: bool, abbr: String) -> i64 {
    z.offs.push(off);
    z.dst.push(dst);
    z.abbrs.push(abbr);
    z.offs.len() as i64 - 1
}

fn parse_rule(z: &mut Zone, s: &[u8]) -> Option<()> {
    let mut p = P { b: s, i: 0 };
    let std_name = p.name()?;
    let std_off = -p.hms(24)?;
    let st = add_type(z, std_off, false, std_name);
    if p.i == s.len() {
        z.rule = vec![st, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        return Some(());
    }
    let dst_name = p.name()?;
    let dst_off = if p.peek() != b',' && p.i < s.len() { -p.hms(24)? } else { std_off + 3600 };
    let dt = add_type(z, dst_off, true, dst_name);
    let mut rule = vec![st, dt];
    if p.i == s.len() {
        rule.extend([0, 3, 2, 0, 7200, 0, 11, 1, 0, 7200]);
    } else {
        for _ in 0..2 {
            if p.peek() != b',' {
                return None;
            }
            p.i += 1;
            p.date(&mut rule)?;
        }
        if p.i != s.len() {
            return None;
        }
    }
    z.rule = rule;
    Some(())
}

fn be(b: &[u8], at: usize, n: usize) -> i64 {
    let mut v: u64 = 0;
    for k in 0..n {
        v = v << 8 | b[at + k] as u64;
    }
    if n == 4 { v as u32 as i32 as i64 } else { v as i64 }
}

pub fn parse_tzif(name: &str, b: &[u8]) -> Option<Zone> {
    let head = |at: usize| -> Option<[u64; 6]> {
        if b.len() < at + 44 || &b[at..at + 4] != b"TZif" {
            return None;
        }
        let mut c = [0u64; 6];
        for (k, x) in c.iter_mut().enumerate() {
            *x = be(b, at + 20 + 4 * k, 4) as u32 as u64;
        }
        Some(c)
    };
    let c1 = head(0)?;
    let (mut c, mut p, mut ts) = (c1, 44usize, 4u64);
    if b[4] != 0 {
        let v1 = c1[3] * 5 + c1[4] * 6 + c1[5] + c1[2] * 8 + c1[1] + c1[0];
        if v1 > b.len() as u64 {
            return None;
        }
        c = head(44 + v1 as usize)?;
        p = 44 + v1 as usize + 44;
        ts = 8;
    }
    let [isut, isstd, leap, timecnt, typecnt, charcnt] = c;
    let need = timecnt * ts + timecnt + typecnt * 6 + charcnt + leap * (ts + 4) + isstd + isut;
    if typecnt == 0 || typecnt > 256 || need > (b.len() - p) as u64 {
        return None;
    }
    let (timecnt, typecnt, charcnt, ts) = (timecnt as usize, typecnt as usize, charcnt as usize, ts as usize);
    let mut z = Zone { name: name.into(), ..Zone::default() };
    for k in 0..timecnt {
        z.trans.push(be(b, p + k * ts, ts));
    }
    p += timecnt * ts;
    for k in 0..timecnt {
        let i = b[p + k] as usize;
        if i >= typecnt {
            return None;
        }
        z.idx.push(i as i64);
    }
    p += timecnt;
    let chars = &b[p + typecnt * 6..p + typecnt * 6 + charcnt];
    for k in 0..typecnt {
        let at = p + k * 6;
        let d = b[at + 5] as usize;
        if d >= charcnt {
            return None;
        }
        let end = chars[d..].iter().position(|x| *x == 0).map_or(charcnt, |e| d + e);
        let abbr = String::from_utf8(chars[d..end].to_vec()).ok()?;
        add_type(&mut z, be(b, at, 4), b[at + 4] != 0, abbr);
    }
    p += typecnt * 6 + charcnt + leap as usize * (ts + 4) + isstd as usize + isut as usize;
    if ts == 8 && p < b.len() {
        if b[p] != b'\n' {
            return None;
        }
        let rest = &b[p + 1..];
        let e = rest.iter().position(|x| *x == b'\n')?;
        if e > 0 {
            parse_rule(&mut z, &rest[..e])?;
        }
    }
    Some(z)
}

fn rule_local(y: i64, r: &[i64]) -> i64 {
    let days = match r[0] {
        0 => {
            let first = days_from_civil(y, r[1], 1);
            let wd = (first + 4).rem_euclid(7);
            let mut day = 1 + (r[3] - wd).rem_euclid(7) + (r[2] - 1) * 7;
            if day > mdays(y, r[1]) {
                day -= 7;
            }
            days_from_civil(y, r[1], day)
        }
        1 => days_from_civil(y, 1, 1) + r[1] - 1 + i64::from(crate::chrono::leap(y) && r[1] >= 60),
        _ => days_from_civil(y, 1, 1) + r[1],
    };
    days * 86400 + r[4]
}

fn rule_type(z: &Zone, s: i64) -> i64 {
    let r = &z.rule;
    if r[1] < 0 {
        return r[0];
    }
    let (so, dso) = (z.offs[r[0] as usize], z.offs[r[1] as usize]);
    let (y, _, _) = civil((s + so).div_euclid(86400));
    let start = rule_local(y, &r[2..7]) - so;
    let end = rule_local(y, &r[7..12]) - dso;
    let dst = if start < end { start <= s && s < end } else { !(end <= s && s < start) };
    if dst { r[1] } else { r[0] }
}

impl Zone {
    pub fn type_at(&self, t_ms: i64) -> usize {
        let s = t_ms.div_euclid(1000);
        let n = self.trans.len();
        if n == 0 || s < self.trans[0] {
            return if n == 0 && !self.rule.is_empty() { rule_type(self, s) as usize } else { 0 };
        }
        let (mut lo, mut hi) = (0, n);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.trans[mid] <= s {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == n && !self.rule.is_empty() {
            return rule_type(self, s) as usize;
        }
        self.idx[lo - 1] as usize
    }

    pub fn offset_ms(&self, t: i64) -> i64 {
        self.offs[self.type_at(t)] * 1000
    }

    pub fn local(&self, t: i64) -> Option<i64> {
        t.checked_add(self.offset_ms(t))
    }

    pub fn utc(&self, w: i64) -> Option<Option<i64>> {
        let two = 2 * 86_400_000;
        let probes = [w.checked_sub(two)?, w, w.checked_add(two)?];
        let mut best: Option<i64> = None;
        for q in probes {
            let o = self.offset_ms(q);
            let t = w.checked_sub(o)?;
            if self.offset_ms(t) == o && best.is_none_or(|b| t < b) {
                best = Some(t);
            }
        }
        Some(best)
    }
}

/// Replaces `%z` and `%Z` for `chrono::strftime`; other directives pass through.
pub fn zone_pattern(pat: &str, off_ms: i64, abbr: &str) -> String {
    let mut out = String::new();
    let mut it = pat.chars();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('z') => out += &offset_name(off_ms / 1000).replace(':', ""),
            Some('Z') => out += &abbr.replace('%', "%%"),
            Some(d) => {
                out.push('%');
                out.push(d);
            }
            None => out.push('%'),
        }
    }
    out
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.len() <= 255 && !n.starts_with('/') && n.split('/').all(|p| !p.is_empty() && p != "." && p != "..") && n.bytes().all(|c| c.is_ascii_alphanumeric() || b"/_-+.".contains(&c))
}

pub fn load(name: &str) -> Result<Zone, String> {
    if let Some(secs) = parse_fixed(name) {
        return Ok(fixed(if secs == 0 { "UTC" } else { name }, secs));
    }
    if !valid_name(name) {
        return Err(format!("{name}: unknown time zone"));
    }
    let dir = std::env::var("TZDIR").ok().filter(|d| !d.is_empty()).unwrap_or_else(|| "/usr/share/zoneinfo".into());
    let bytes = std::fs::read(format!("{dir}/{name}")).map_err(|_| format!("{name}: unknown time zone"))?;
    parse_tzif(name, &bytes).ok_or_else(|| format!("{name}: invalid time zone data"))
}

pub fn load_local() -> Result<Zone, String> {
    let bytes = std::fs::read("/etc/localtime").map_err(|_| "localtime: unknown time zone".to_string())?;
    let link = std::fs::read_link("/etc/localtime").ok().and_then(|p| p.to_str().map(str::to_string)).unwrap_or_default();
    let name = link.rfind("zoneinfo/").map_or("localtime", |i| &link[i + 9..]);
    parse_tzif(name, &bytes).ok_or_else(|| "localtime: invalid time zone data".to_string())
}
