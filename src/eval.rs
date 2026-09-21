use crate::{embed, extractor::Function, search};

/// (السؤال، أسماء الدوال المقبولة كإجابة صحيحة)
pub const CASES: &[(&str, &[&str])] = &[
    ("stop a running task", &["abort_all", "abort", "shutdown", "abort_matching"]),
    ("cancel token", &["cancel", "cancelled", "is_cancelled"]),
    ("wait until all work finishes", &["wait", "join_all", "shutdown"]),
    ("check if the queue has no items", &["is_empty", "len"]),
    ("add a new delayed item", &["insert", "insert_at"]),
    ("create a child of a token", &["child_token"]),
    ("remove an item before it expires", &["remove", "try_remove"]),
    ("run a future on a specific thread", &["spawn_pinned", "spawn_pinned_by_idx"]),
];

fn rank_of(hits: &[search::Hit], answers: &[&str]) -> Option<usize> {
    hits.iter()
        .position(|h| answers.contains(&h.func.name.as_str()))
}

pub fn run(
    funcs: &[Function],
    vecs: &[Vec<f32>],
    emb: &mut embed::Embedder,
) -> anyhow::Result<()> {
    let names = ["lexical", "semantic", "hybrid"];
    let mut top1 = [0usize; 3];
    let mut top3 = [0usize; 3];
    let mut top5 = [0usize; 3];

    for (q, answers) in CASES {
        let qv = emb.embed(vec![q.to_string()])?.remove(0);
        let results = [
            search::search(funcs, q, 10),
            search::semantic_search(funcs, vecs, &qv, 10),
            search::hybrid_search(funcs, vecs, q, &qv, 10),
        ];
        print!("{:<40}", q);
        for (i, hits) in results.iter().enumerate() {
            let r = rank_of(hits, answers);
            match r {
                Some(p) => print!(" {}:#{:<3}", &names[i][..3], p + 1),
                None => print!(" {}:--  ", &names[i][..3]),
            }
            if let Some(p) = r {
                if p < 1 { top1[i] += 1; }
                if p < 3 { top3[i] += 1; }
                if p < 5 { top5[i] += 1; }
            }
        }
        println!();
    }

        let n = CASES.len();
    println!("\n{} أسئلة", n);
    println!("{:<10} {:>7} {:>7} {:>7}", "mode", "top1", "top3", "top5");
    for i in 0..3 {
        println!(
            "{:<10} {:>6}% {:>6}% {:>6}%",
            names[i],
            top1[i] * 100 / n,
            top3[i] * 100 / n,
            top5[i] * 100 / n
        );
    }
    Ok(())
}