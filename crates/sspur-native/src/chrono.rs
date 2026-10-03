pub const DAY: i64 = 86_400_000;
pub const MAX_YEAR: i64 = 1_000_000;

pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

pub fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

pub fn mdays(y: i64, m: i64) -> i64 {
    match m {
        2 if leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

pub struct Parts {
    pub y: i64,
    pub mo: i64,
    pub d: i64,
    pub h: i64,
    pub mi: i64,
    pub s: i64,
    pub ms: i64,
    pub wd: i64,
    pub yd: i64,
}

pub fn parts(t: i64) -> Parts {
    let z = t.div_euclid(DAY);
    let r = t.rem_euclid(DAY);
    let (y, mo, d) = civil(z);
    Parts { y, mo, d, h: r / 3_600_000, mi: r / 60_000 % 60, s: r / 1000 % 60, ms: r % 1000, wd: (z + 3).rem_euclid(7) + 1, yd: z - days_from_civil(y, 1, 1) + 1 }
}

pub fn year_str(y: i64) -> String {
    if (0..=9999).contains(&y) {
        format!("{y:04}")
    } else if y < 0 {
        format!("-{:04}", -y)
    } else {
        format!("+{y}")
    }
}

pub fn iso(t: i64) -> String {
    let p = parts(t);
    let frac = if p.ms != 0 { format!(".{:03}", p.ms) } else { String::new() };
    format!("{}-{:02}-{:02}T{:02}:{:02}:{:02}{frac}Z", year_str(p.y), p.mo, p.d, p.h, p.mi, p.s)
}

pub fn dur_str(d: i64) -> String {
    if d == 0 {
        return "0s".into();
    }
    let u = d.unsigned_abs();
    let mut s = String::new();
    if u < 1000 {
        s = format!("{u}ms");
    } else {
        let (h, m, sec, ms) = (u / 3_600_000, u / 60_000 % 60, u / 1000 % 60, u % 1000);
        if h > 0 {
            s += &format!("{h}h");
        }
        if h > 0 || m > 0 {
            s += &format!("{m}m");
        }
        s += &sec.to_string();
        if ms > 0 {
            s += format!(".{ms:03}").trim_end_matches('0');
        }
        s.push('s');
    }
    if d < 0 { format!("-{s}") } else { s }
}

pub fn civil_ms(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64) -> Option<i64> {
    let ok = (-MAX_YEAR..=MAX_YEAR).contains(&y) && (1..=12).contains(&mo) && d >= 1 && d <= mdays(y, mo) && (0..24).contains(&h) && (0..60).contains(&mi) && (0..60).contains(&s);
    ok.then(|| days_from_civil(y, mo, d) * DAY + h * 3_600_000 + mi * 60_000 + s * 1000)
}

pub fn parse(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    let take = |j: &mut usize, n: usize| -> Option<i64> {
        if *j + n > b.len() || !b[*j..*j + n].iter().all(u8::is_ascii_digit) {
            return None;
        }
        let v = b[*j..*j + n].iter().fold(0i64, |a, c| a * 10 + i64::from(c - b'0'));
        *j += n;
        Some(v)
    };
    let lit = |i: &mut usize, c: u8| -> bool {
        if *i < b.len() && b[*i] == c {
            *i += 1;
            true
        } else {
            false
        }
    };
    let mut j = 0;
    let y = take(&mut j, 4)?;
    if !lit(&mut j, b'-') {
        return None;
    }
    let mo = take(&mut j, 2)?;
    if !lit(&mut j, b'-') {
        return None;
    }
    let d = take(&mut j, 2)?;
    let (mut h, mut mi, mut sec, mut ms, mut off) = (0, 0, 0, 0, 0);
    if j < b.len() {
        if !(lit(&mut j, b'T') || lit(&mut j, b't') || lit(&mut j, b' ')) {
            return None;
        }
        h = take(&mut j, 2)?;
        if !lit(&mut j, b':') {
            return None;
        }
        mi = take(&mut j, 2)?;
        if lit(&mut j, b':') {
            sec = take(&mut j, 2)?;
            if lit(&mut j, b'.') || lit(&mut j, b',') {
                let st = j;
                while j < b.len() && b[j].is_ascii_digit() {
                    if j - st < 3 {
                        ms = ms * 10 + i64::from(b[j] - b'0');
                    }
                    j += 1;
                }
                let n = j - st;
                if n == 0 || n > 9 {
                    return None;
                }
                for _ in n..3 {
                    ms *= 10;
                }
            }
        }
        if !(lit(&mut j, b'Z') || lit(&mut j, b'z')) && j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            let sign = if b[j] == b'-' { -1 } else { 1 };
            j += 1;
            let oh = take(&mut j, 2)?;
            let colon = lit(&mut j, b':');
            let om = if colon || j < b.len() { take(&mut j, 2)? } else { 0 };
            if oh > 23 || om > 59 {
                return None;
            }
            off = sign * (oh * 3_600_000 + om * 60_000);
        }
    }
    if j != b.len() {
        return None;
    }
    Some(civil_ms(y, mo, d, h, mi, sec)? + ms - off)
}

const WD: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MO: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

pub fn strftime(t: i64, pat: &str) -> Option<String> {
    let p = parts(t);
    let mut out = String::new();
    let mut it = pat.chars();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('Y') => out += &year_str(p.y),
            Some('m') => out += &format!("{:02}", p.mo),
            Some('d') => out += &format!("{:02}", p.d),
            Some('H') => out += &format!("{:02}", p.h),
            Some('M') => out += &format!("{:02}", p.mi),
            Some('S') => out += &format!("{:02}", p.s),
            Some('L') => out += &format!("{:03}", p.ms),
            Some('j') => out += &format!("{:03}", p.yd),
            Some('u') => out += &p.wd.to_string(),
            Some('a') => out += &WD[(p.wd - 1) as usize][..3],
            Some('A') => out += WD[(p.wd - 1) as usize],
            Some('b') => out += &MO[(p.mo - 1) as usize][..3],
            Some('B') => out += MO[(p.mo - 1) as usize],
            Some('F') => out += &format!("{}-{:02}-{:02}", year_str(p.y), p.mo, p.d),
            Some('T') => out += &format!("{:02}:{:02}:{:02}", p.h, p.mi, p.s),
            Some('%') => out.push('%'),
            _ => return None,
        }
    }
    Some(out)
}
