mod embed;
mod eval;
mod extractor;
mod search;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "codemap", about = "Local-first code search for Rust projects")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Mode {
    /// بحث بالكلمات فقط
    Lexical,
    /// بحث بالمعنى فقط
    Semantic,
    /// دمج الاثنين (الأفضل)
    Hybrid,
}

#[derive(Subcommand)]
enum Cmd {
    /// اعرض كل الدوال
    List { path: PathBuf },
    /// من يستدعي هذه الدالة؟
    Callers { path: PathBuf, name: String },
    /// ابنِ فهرس المعنى مسبقاً
    Index { path: PathBuf },
    /// ابحث بجملة عادية
    Ask {
        path: PathBuf,
        question: String,
        /// طريقة البحث
        #[arg(short, long, value_enum, default_value_t = Mode::Hybrid)]
        mode: Mode,
        /// عدد النتائج
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    /// قياس جودة البحث على أسئلة معروفة
    Eval { path: PathBuf },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let start = Instant::now();

    match args.cmd {
        Cmd::List { path } => {
            let fns = extractor::scan_project(&path)?;
            for f in &fns {
                println!("{}:{}  {}", f.file, f.line, f.full_name());
            }
            println!("\n✅ {} دالة في {:?}", fns.len(), start.elapsed());
        }
        Cmd::Callers { path, name } => {
            let fns = extractor::scan_project(&path)?;
            let mut found = 0;
            for f in fns.iter().filter(|f| f.calls.iter().any(|c| c == &name)) {
                println!("{}:{}  {}", f.file, f.line, f.full_name());
                found += 1;
            }
            println!("\n🔎 {} دالة تستدعي `{}` ({:?})", found, name, start.elapsed());
        }
        Cmd::Index { path } => {
            let fns = extractor::scan_project(&path)?;
            let mut emb = embed::Embedder::new()?;
            embed::load_or_build(&path, &fns, &mut emb)?;
            println!("✅ تم الفهرسة في {:?}", start.elapsed());
        }
        Cmd::Ask {
            path,
            question,
            mode,
            limit,
        } => {
            let fns = extractor::scan_project(&path)?;
            let hits = match mode {
                Mode::Lexical => search::search(&fns, &question, limit),
                Mode::Semantic | Mode::Hybrid => {
                    let mut emb = embed::Embedder::new()?;
                    let vecs = embed::load_or_build(&path, &fns, &mut emb)?;
                    let q = emb.embed(vec![question.clone()])?.remove(0);
                    if matches!(mode, Mode::Semantic) {
                        search::semantic_search(&fns, &vecs, &q, limit)
                    } else {
                        search::hybrid_search(&fns, &vecs, &question, &q, limit)
                    }
                }
            };

            if hits.is_empty() {
                println!("لا توجد نتائج. جرّب كلمات أخرى.");
            }
            for h in hits {
                println!(
                    "{:>6.3}  {}:{}  {}",
                    h.score,
                    h.func.file,
                    h.func.line,
                    h.func.full_name()
                );
            }
            println!("\n⏱ {:?}", start.elapsed());
        }
        Cmd::Eval { path } => {
            let fns = extractor::scan_project(&path)?;
            let mut emb = embed::Embedder::new()?;
            let vecs = embed::load_or_build(&path, &fns, &mut emb)?;
            eval::run(&fns, &vecs, &mut emb)?;
        }
    }
    Ok(())
}