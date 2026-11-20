//! Deterministic synthetic chat content: message bodies of realistic length,
//! host-formatted header lines and anchors.

/// SplitMix64: small, fast, deterministic; good enough for synthetic data.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    /// Inclusive range.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi - lo + 1)
    }
}

const WORDS: &[&str] = &[
    "the", "trip", "friday", "works", "can", "we", "move", "to", "meeting", "code", "build",
    "deploy", "lunch", "tomorrow", "sounds", "good", "i", "think", "maybe", "later", "calendar",
    "note", "plugin", "space", "host", "record", "file", "yes", "no", "thanks", "please",
    "check", "this", "link", "review", "merge", "branch", "test", "again", "after", "before",
    "morning", "evening", "what", "about", "see", "you", "and", "a", "it", "is", "for", "on",
];

pub const NAMES: &[&str] = &["Lee", "Sam", "Ana", "Kim", "Jo", "Ravi", "Mia", "Tomás"];

fn sentence(rng: &mut Rng, target: usize, out: &mut String) {
    let start = out.len();
    while out.len() - start < target {
        if out.len() > start {
            out.push(' ');
        }
        out.push_str(WORDS[rng.below(WORDS.len() as u64) as usize]);
    }
    if let Some(c) = out[start..].chars().next() {
        let up: String = c.to_uppercase().collect();
        out.replace_range(start..start + c.len_utf8(), &up);
    }
    out.push('.');
}

fn paragraph(rng: &mut Rng, target: usize, out: &mut String) {
    let start = out.len();
    while out.len() - start < target {
        if out.len() > start {
            out.push(' ');
        }
        let t = rng.range(20, 90) as usize;
        sentence(rng, t, out);
    }
}

/// One message body. Mix (by count): 66% one short line, 25% a paragraph,
/// 6% several paragraphs, 3% a fenced code block.
pub fn body(rng: &mut Rng) -> String {
    let mut s = String::new();
    match rng.below(100) {
        0..66 => {
            let t = rng.range(8, 80) as usize;
            sentence(rng, t, &mut s)
        }
        66..91 => {
            let t = rng.range(100, 400) as usize;
            paragraph(rng, t, &mut s)
        }
        91..97 => {
            let total = rng.range(400, 1500) as usize;
            while s.len() < total {
                if !s.is_empty() {
                    s.push_str("\n\n");
                }
                let t = rng.range(120, 400) as usize;
                paragraph(rng, t, &mut s);
            }
        }
        _ => {
            s.push_str("Here is the change:\n\n```rust\n");
            for i in 0..rng.range(5, 40) {
                let w = WORDS[rng.below(WORDS.len() as u64) as usize];
                s.push_str(&format!("    let {w}_{i} = {w}(&args[{i}])?;\n"));
            }
            s.push_str("```");
        }
    }
    s
}

/// A body of roughly `len` bytes (at least one word).
pub fn body_of_len(rng: &mut Rng, len: usize) -> String {
    let len = len.max(4);
    let mut s = String::new();
    paragraph(rng, len, &mut s);
    s.truncate(len - 1); // the words are ASCII
    let t = s.trim_end().len();
    s.truncate(t);
    s.push('.');
    s
}

const CROCKFORD: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// A 16-character Crockford anchor (80 random bits), as ADR 0056 block IDs.
pub fn anchor(rng: &mut Rng) -> String {
    let mut a = rng.next() as u128 | ((rng.next() as u128) << 64);
    (0..16)
        .map(|_| {
            let c = CROCKFORD[(a & 31) as usize] as char;
            a >>= 5;
            c
        })
        .collect()
}

pub fn is_anchor(s: &[u8]) -> bool {
    s.len() == 16 && s.iter().all(|c| CROCKFORD.contains(c))
}

/// Days since 1970-01-01 to (year, month, day) (H. Hinnant's algorithm).
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// (year, month, day) to days since 1970-01-01.
pub fn days(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Name and time from a header line (`**Lee** · 2026-11-20 17:02 ^…`), for
/// records an outside editor added.
pub fn parse_header(line: &[u8]) -> Option<(String, i64)> {
    let s = std::str::from_utf8(line).ok()?;
    let (name, rest) = s.strip_prefix("**")?.split_once("** · ")?;
    let t = rest.get(..16)?;
    let n = |r: std::ops::Range<usize>| t.get(r)?.parse::<i64>().ok();
    let at = days(n(0..4)?, n(5..7)? as u32, n(8..10)? as u32) * 86_400 + n(11..13)? * 3600 + n(14..16)? * 60;
    Some((name.to_string(), at))
}

/// Month number (year * 12 + month - 1) of a Unix time in seconds: the
/// segment key for layout C.
pub fn month_of(at: i64) -> i64 {
    let (y, m, _) = civil(at.div_euclid(86_400));
    y * 12 + i64::from(m) - 1
}

pub fn month_name(month: i64) -> String {
    format!("{:04}-{:02}", month / 12, month % 12 + 1)
}

/// `**Lee** · 2026-11-20 17:02 ^k2m8q1w4zre7k3f9`
pub fn header(name: &str, at: i64, anchor: &str) -> String {
    let (y, m, d) = civil(at.div_euclid(86_400));
    let s = at.rem_euclid(86_400);
    format!("**{name}** · {y:04}-{m:02}-{d:02} {:02}:{:02} ^{anchor}", s / 3600, s / 60 % 60)
}

/// True if `line` has the host header shape; used both to escape bodies on
/// write and to find records when re-anchoring.
pub fn header_anchor(line: &[u8]) -> Option<&[u8]> {
    if line.len() < 30 || !line.starts_with(b"**") {
        return None;
    }
    let (rest, tail) = line.split_at(line.len() - 18);
    if &tail[..2] != b" ^" || !is_anchor(&tail[2..]) {
        return None;
    }
    // "** · " (the middle dot is two bytes in UTF-8).
    rest.windows(6).any(|w| w == "** · ".as_bytes()).then_some(&tail[2..])
}

/// Escapes body lines that look like a header, so a plugin can never forge
/// a record boundary.
pub fn escape_body(body: &str) -> String {
    if !body.lines().any(|l| header_anchor(l.as_bytes()).is_some()) {
        return body.trim_end().to_string();
    }
    body.trim_end()
        .lines()
        .map(|l| if header_anchor(l.as_bytes()).is_some() { format!("\\{l}") } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\n")
}
