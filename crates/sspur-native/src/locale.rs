//! Bundled locale rules shared by both tiers: the interpreter calls these functions and
//! `c_tables` emits the same data for the `locale` section of `std_rt.c`.
use std::cmp::Ordering;
use std::sync::OnceLock;

pub struct Loc {
    pub tag: &'static str,
    pub dec: &'static str,
    pub group: &'static str,
    pub indian: bool,
    pub pats: [&'static str; 3],
    pub am: &'static str,
    pub pm: &'static str,
    pub months: [&'static str; 12],
}

const EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

pub const LOCALES: [Loc; 6] = [
    Loc { tag: "en-US", dec: ".", group: ",", indian: false, pats: ["%m/%d/%Y", "%B %d, %Y", "%I:%N %p"], am: "AM", pm: "PM", months: EN },
    Loc { tag: "en-GB", dec: ".", group: ",", indian: false, pats: ["%D/%M/%Y", "%d %B %Y", "%H:%N"], am: "am", pm: "pm", months: EN },
    Loc {
        tag: "de-DE",
        dec: ",",
        group: ".",
        indian: false,
        pats: ["%D.%M.%Y", "%d. %B %Y", "%H:%N"],
        am: "AM",
        pm: "PM",
        months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
    },
    Loc {
        tag: "fr-FR",
        dec: ",",
        group: "\u{202f}",
        indian: false,
        pats: ["%D/%M/%Y", "%d %B %Y", "%H:%N"],
        am: "AM",
        pm: "PM",
        months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
    },
    Loc {
        tag: "ja-JP",
        dec: ".",
        group: ",",
        indian: false,
        pats: ["%Y/%M/%D", "%Y年%m月%d日", "%k:%N"],
        am: "午前",
        pm: "午後",
        months: ["1月", "2月", "3月", "4月", "5月", "6月", "7月", "8月", "9月", "10月", "11月", "12月"],
    },
    Loc {
        tag: "hi-IN",
        dec: ".",
        group: ",",
        indian: true,
        pats: ["%d/%m/%Y", "%d %B %Y", "%I:%N %p"],
        am: "am",
        pm: "pm",
        months: ["जनवरी", "फ़रवरी", "मार्च", "अप्रैल", "मई", "जून", "जुलाई", "अगस्त", "सितंबर", "अक्तूबर", "नवंबर", "दिसंबर"],
    },
];

pub fn index(tag: &str) -> Option<usize> {
    LOCALES.iter().position(|l| l.tag == tag)
}

fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

/// Regroups a plain `[-]digits[.digits]` number with the locale's separators; anything else
/// (`inf`, `NaN`) is returned unchanged.
pub fn localize(i: usize, s: &str) -> String {
    let l = &LOCALES[i];
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let (int, frac) = match body.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (body, None),
    };
    if !digits(int) || frac.is_some_and(|f| !digits(f)) {
        return s.to_string();
    }
    let mut groups: Vec<&str> = Vec::new();
    let mut end = int.len();
    let mut size = 3;
    while end > size {
        groups.push(&int[end - size..end]);
        end -= size;
        if l.indian {
            size = 2;
        }
    }
    groups.push(&int[..end]);
    groups.reverse();
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out += &groups.join(l.group);
    if let Some(f) = frac {
        out += l.dec;
        out += f;
    }
    out
}

/// `kind` 0 is the numeric date, 1 the long date, 2 the time of day.
pub fn format_time(i: usize, kind: usize, t: i64) -> String {
    let l = &LOCALES[i];
    let p = crate::chrono::parts(t);
    let mut out = String::new();
    let mut it = l.pats[kind].chars();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('d') => out += &p.d.to_string(),
            Some('D') => out += &format!("{:02}", p.d),
            Some('m') => out += &p.mo.to_string(),
            Some('M') => out += &format!("{:02}", p.mo),
            Some('Y') => out += &p.y.to_string(),
            Some('B') => out += l.months[(p.mo - 1) as usize],
            Some('H') => out += &format!("{:02}", p.h),
            Some('k') => out += &p.h.to_string(),
            Some('I') => out += &(if p.h % 12 == 0 { 12 } else { p.h % 12 }).to_string(),
            Some('N') => out += &format!("{:02}", p.mi),
            Some('p') => out += if p.h < 12 { l.am } else { l.pm },
            _ => {}
        }
    }
    out
}

/// Latin-1 Supplement and Latin Extended-A, U+00C0 to U+017F: base letters (case marks
/// uppercase) and a mark code for the secondary weight; `-` is not a letter.
const LATIN: &str = "A:g A:a A:c A:t A:d A:r AE:l C:e E:g E:a E:c E:d I:g I:a I:c I:d \
D:s N:t O:g O:a O:c O:t O:d - O:s U:g U:a U:c U:d Y:a Þ:. ss:l \
a:g a:a a:c a:t a:d a:r ae:l c:e e:g e:a e:c e:d i:g i:a i:c i:d \
d:s n:t o:g o:a o:c o:t o:d - o:s u:g u:a u:c u:d y:a þ:. y:d \
A:m a:m A:b a:b A:k a:k C:a c:a C:c c:c C:o c:o C:v c:v D:v d:v \
D:s d:s E:m e:m E:b e:b E:o e:o E:k e:k E:v e:v G:c g:c G:b g:b \
G:o g:o G:e g:e H:c h:c H:s h:s I:t i:t I:m i:m I:b i:b I:k i:k \
I:o i:x IJ:l ij:l J:c j:c K:e k:e k:x L:a l:a L:e l:e L:v l:v L:x \
l:x L:s l:s N:a n:a N:e n:e N:v n:v n:x N:x n:x O:m o:m O:b o:b \
O:h o:h OE:l oe:l R:a r:a R:e r:e R:v r:v S:a s:a S:c s:c S:e s:e \
S:v s:v T:e t:e T:v t:v T:s t:s U:t u:t U:m u:m U:b u:b U:r u:r \
U:h u:h U:k u:k W:c w:c Y:c y:c Y:d Z:a z:a Z:o z:o Z:v z:v s:x";

/// Secondary weights in DUCET order for the marks the table uses; 0x20 is the common weight.
fn mark_weight(c: u8) -> u8 {
    match c {
        b'a' => 0x24,
        b'g' => 0x25,
        b'b' => 0x26,
        b'c' => 0x27,
        b'v' => 0x28,
        b'r' => 0x29,
        b'd' => 0x2B,
        b'h' => 0x2C,
        b't' => 0x2D,
        b'o' => 0x2E,
        b's' => 0x2F,
        b'e' => 0x30,
        b'k' => 0x31,
        b'm' => 0x32,
        b'l' => 0x33,
        b'x' => 0x3F,
        _ => 0,
    }
}

/// Combining marks U+0300 to U+036F.
fn comb_weight(cp: u32) -> u8 {
    mark_weight(match cp {
        0x300 => b'g',
        0x301 => b'a',
        0x302 => b'c',
        0x303 => b't',
        0x304 => b'm',
        0x306 => b'b',
        0x307 => b'o',
        0x308 => b'd',
        0x30A => b'r',
        0x30B => b'h',
        0x30C => b'v',
        0x327 => b'e',
        0x328 => b'k',
        0x335..=0x338 => b's',
        _ => b'x',
    })
}

fn letter(c: char) -> (u8, bool) {
    match c {
        'a'..='z' => (c as u8 - b'a' + 1, false),
        'A'..='Z' => (c as u8 - b'A' + 1, true),
        'þ' => (27, false),
        _ => (27, true),
    }
}

/// `[base1, base2, secondary, upper]` per code point from U+00C0.
fn latin() -> &'static Vec<[u8; 4]> {
    static T: OnceLock<Vec<[u8; 4]>> = OnceLock::new();
    T.get_or_init(|| {
        LATIN
            .split_whitespace()
            .map(|tok| {
                if tok == "-" {
                    return [0, 0, 0, 0];
                }
                let (base, mark) = tok.split_once(':').unwrap();
                let mut cs = base.chars();
                let (b1, up) = letter(cs.next().unwrap());
                let b2 = cs.next().map_or(0, |c| letter(c).0);
                [b1, b2, mark_weight(mark.as_bytes()[0]), u8::from(up)]
            })
            .collect()
    })
}

fn elements(s: &str) -> Vec<[u32; 3]> {
    let mut out = Vec::with_capacity(s.len());
    for c in s.chars() {
        let cp = c as u32;
        match cp {
            0..0x20 | 0x7F..=0x9F => {}
            0x61..=0x7A => out.push([0x2000 + cp - 0x60, 0x20, 2]),
            0x41..=0x5A => out.push([0x2000 + cp - 0x40, 0x20, 8]),
            0x30..=0x39 => out.push([0x1000 + cp - 0x30, 0x20, 2]),
            0x966..=0x96F => out.push([0x1000 + cp - 0x966, 0x20, 4]),
            0xC0..=0x17F => {
                let [b1, b2, m, up] = latin()[(cp - 0xC0) as usize];
                if b1 == 0 {
                    out.push([0x100 + cp, 0x20, 2]);
                    continue;
                }
                let t = if up == 1 { 8 } else { 2 };
                out.push([0x2000 + u32::from(b1), 0x20, t]);
                if m != 0 {
                    out.push([0, u32::from(m), 2]);
                }
                if b2 != 0 {
                    out.push([0x2000 + u32::from(b2), 0x20, t]);
                }
            }
            0..0xC0 => out.push([0x100 + cp, 0x20, 2]),
            0x300..=0x36F => out.push([0, u32::from(comb_weight(cp)), 2]),
            _ => out.push([0x10000 + cp, 0x20, 2]),
        }
    }
    out
}

/// Three-level comparison (base letters, then accents, then case) and code point order last,
/// so only equal strings compare equal. The same rules serve every bundled locale.
pub fn compare(a: &str, b: &str) -> Ordering {
    let (x, y) = (elements(a), elements(b));
    for level in 0..3 {
        let fx = x.iter().map(|e| e[level]).filter(|w| *w != 0);
        let fy = y.iter().map(|e| e[level]).filter(|w| *w != 0);
        match fx.cmp(fy) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    a.as_bytes().cmp(b.as_bytes())
}

fn c_str(s: &str) -> String {
    let mut out = String::from("\"");
    for b in s.bytes() {
        match b {
            b'"' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            0x20..=0x7E => out.push(b as char),
            _ => out += &format!("\\{b:03o}"),
        }
    }
    out.push('"');
    out
}

fn c_list<'a>(xs: impl Iterator<Item = &'a str>) -> String {
    xs.map(c_str).collect::<Vec<_>>().join(", ")
}

/// The data tables the C runtime section `locale` reads.
pub fn c_tables() -> String {
    let ls = &LOCALES;
    let mut s = String::new();
    s += &format!("static const char* const ss_loc_tag[6] = {{{}}};\n", c_list(ls.iter().map(|l| l.tag)));
    s += &format!("static const char* const ss_loc_dec[6] = {{{}}};\n", c_list(ls.iter().map(|l| l.dec)));
    s += &format!("static const char* const ss_loc_grp[6] = {{{}}};\n", c_list(ls.iter().map(|l| l.group)));
    s += &format!("static const int ss_loc_ind[6] = {{{}}};\n", ls.iter().map(|l| if l.indian { "1" } else { "0" }).collect::<Vec<_>>().join(", "));
    s += &format!("static const char* const ss_loc_pat[6][3] = {{{}}};\n", ls.iter().map(|l| format!("{{{}}}", c_list(l.pats.iter().copied()))).collect::<Vec<_>>().join(", "));
    s += &format!("static const char* const ss_loc_am[6] = {{{}}};\n", c_list(ls.iter().map(|l| l.am)));
    s += &format!("static const char* const ss_loc_pm[6] = {{{}}};\n", c_list(ls.iter().map(|l| l.pm)));
    s += &format!("static const char* const ss_loc_mon[6][12] = {{{}}};\n", ls.iter().map(|l| format!("{{{}}}", c_list(l.months.iter().copied()))).collect::<Vec<_>>().join(", "));
    let lat: Vec<String> = latin().iter().map(|e| format!("{{{}, {}, {}, {}}}", e[0], e[1], e[2], e[3])).collect();
    s += &format!("static const unsigned char ss_coll_lat[{}][4] = {{{}}};\n", lat.len(), lat.join(", "));
    let comb: Vec<String> = (0x300..=0x36F).map(|cp| comb_weight(cp).to_string()).collect();
    s += &format!("static const unsigned char ss_coll_comb[112] = {{{}}};\n", comb.join(", "));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_table_covers_c0_to_17f() {
        assert_eq!(latin().len(), 0x180 - 0xC0);
    }

    #[test]
    fn collation_orders_accents_and_case_after_letters() {
        let mut xs = vec!["zebra", "Äpfel", "apfel", "Apfel", "äpfel", "abc", "Zürich", "zurich", "côte", "cote", "coté", "côté", "straße", "strasse", "Ab", "ab", "a b", "10", "9"];
        xs.sort_by(|a, b| compare(a, b));
        assert_eq!(xs, ["10", "9", "a b", "ab", "Ab", "abc", "apfel", "Apfel", "äpfel", "Äpfel", "cote", "coté", "côte", "côté", "strasse", "straße", "zebra", "zurich", "Zürich"]);
        assert_eq!(compare("e\u{301}", "é"), "e\u{301}".as_bytes().cmp("é".as_bytes()));
        assert_eq!(compare("ꙮ", "ꙮ"), Ordering::Equal);
    }

    #[test]
    fn numbers_group_per_locale() {
        assert_eq!(localize(0, "-1234567.891"), "-1,234,567.891");
        assert_eq!(localize(2, "1234567.5"), "1.234.567,5");
        assert_eq!(localize(3, "1234"), "1\u{202f}234");
        assert_eq!(localize(5, "123456789"), "12,34,56,789");
        assert_eq!(localize(5, "999"), "999");
        assert_eq!(localize(0, "inf"), "inf");
    }
}
