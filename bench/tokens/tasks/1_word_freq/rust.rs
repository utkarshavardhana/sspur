use std::collections::HashMap;

pub fn top_words(text: &str, k: usize) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for w in text
        .to_lowercase()
        .split(|c: char| !c.is_ascii_lowercase())
        .filter(|w| !w.is_empty())
    {
        *counts.entry(w.to_string()).or_insert(0) += 1;
    }
    let mut out: Vec<_> = counts.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out.truncate(k);
    out
}
