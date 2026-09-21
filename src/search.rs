use crate::extractor::Function;

pub struct Hit<'a> {
    pub func: &'a Function,
    pub score: f32,
}

/// دوال تنفيذ traits المتكررة، لا تحمل معنى عند البحث
const NOISE: [&str; 9] = [
    "fmt", "clone", "eq", "hash", "drop", "default", "from", "as_ref", "borrow",
];

/// نفصل snake_case و CamelCase إلى كلمات صغيرة
fn tokenize(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            if ch.is_uppercase() && prev_lower && !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev_lower = ch.is_lowercase();
            cur.extend(ch.to_lowercase());
        } else {
            if !cur.is_empty() {
                words.push(std::mem::take(&mut cur));
            }
            prev_lower = false;
        }
    }
    if !cur.is_empty() {
        words.push(cur);
    }
    words
}

pub fn search<'a>(funcs: &'a [Function], query: &str, limit: usize) -> Vec<Hit<'a>> {
    let q = tokenize(query);

    let mut hits: Vec<Hit> = funcs
        .iter()
        .filter_map(|f| {
            let fn_words = tokenize(&f.name);
            let owner_words = f.owner.as_deref().map(tokenize).unwrap_or_default();
            let body_lower = f.body.to_lowercase();
            let mut score = 0.0;

            for w in &q {
                // اسم الدالة: الأهم
                if fn_words.iter().any(|n| n == w) {
                    score += 4.0;
                } else if fn_words.iter().any(|n| n.contains(w.as_str())) {
                    score += 2.0;
                }
                // اسم النوع: أقل أهمية
                if owner_words.iter().any(|n| n == w) {
                    score += 1.0;
                }
                // نص الدالة: الأضعف
                if body_lower.contains(w.as_str()) {
                    score += 0.5;
                }
            }

            // عقوبة الدوال العامة المتكررة
            if NOISE.contains(&f.name.as_str()) {
                score *= 0.3;
            }
            (score > 0.0).then_some(Hit { func: f, score })
        })
        .collect();

    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(limit);
    hits
}