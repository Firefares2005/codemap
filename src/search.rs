use crate::extractor::Function;
use std::collections::HashMap;

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

/// البحث النصي (بدون ذكاء اصطناعي)
pub fn search<'a>(funcs: &'a [Function], query: &str, limit: usize) -> Vec<Hit<'a>> {
    let q = tokenize(query);

    let mut hits: Vec<Hit> = funcs
        .iter()
        .filter_map(|f| {
            let fn_words = tokenize(&f.name);
            let owner_words = f.owner.as_deref().map(tokenize).unwrap_or_default();
            let body_lower = f.body.to_lowercase();
            let mut score = 0.0;
            let mut matched = 0;

            for w in &q {
                let mut hit = false;
                if fn_words.iter().any(|n| n == w) {
                    score += 4.0;
                    hit = true;
                } else if fn_words.iter().any(|n| n.starts_with(w.as_str())) {
                    score += 2.0;
                    hit = true;
                }
                if owner_words.iter().any(|n| n == w) {
                    score += 1.5;
                    hit = true;
                }
                if body_lower.contains(w.as_str()) {
                    score += 0.5;
                }
                if hit {
                    matched += 1;
                }
            }

            // مكافأة لمن يغطي أكثر كلمات السؤال
            score *= 1.0 + 0.5 * matched as f32;

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

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// البحث بالمعنى: أقرب الدوال للسؤال في فضاء embeddings
pub fn semantic_search<'a>(
    funcs: &'a [Function],
    doc_vecs: &[Vec<f32>],
    query_vec: &[f32],
    limit: usize,
) -> Vec<Hit<'a>> {
    let mut hits: Vec<Hit> = funcs
        .iter()
        .zip(doc_vecs)
        .map(|(f, v)| {
            let mut score = cosine(query_vec, v);
            if NOISE.contains(&f.name.as_str()) {
                score *= 0.5;
            }
            Hit { func: f, score }
        })
        .collect();

    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(limit);
    hits
}

/// البحث الهجين: دمج ترتيب النصي والدلالي بطريقة RRF
pub fn hybrid_search<'a>(
    funcs: &'a [Function],
    doc_vecs: &[Vec<f32>],
    query: &str,
    query_vec: &[f32],
    limit: usize,
) -> Vec<Hit<'a>> {
    const K: f32 = 60.0;
    let pool = 50;

    let lexical = search(funcs, query, pool);
    let semantic = semantic_search(funcs, doc_vecs, query_vec, pool);

    let mut fused: HashMap<*const Function, (f32, &'a Function)> = HashMap::new();
        // الدلالي أقوى في أسئلة اللغة العادية، فنعطيه وزناً أكبر
    for (list, weight) in [(&lexical, 0.5_f32), (&semantic, 1.0_f32)] {
        for (rank, h) in list.iter().enumerate() {
            let e = fused
                .entry(h.func as *const Function)
                .or_insert((0.0, h.func));
            e.0 += weight / (K + rank as f32 + 1.0);
        }
    }

    let mut hits: Vec<Hit> = fused
        .into_values()
        .map(|(score, func)| Hit { func, score })
        .collect();
    hits.sort_by(|a, b| b.score.total_cmp(&a.score));
    hits.truncate(limit);
    hits
}