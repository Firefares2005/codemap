mod extractor;
mod search;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "codemap", about = "Local-first code search for Rust projects")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// اعرض كل الدوال
    List { path: PathBuf },
    /// من يستدعي هذه الدالة؟
    Callers { path: PathBuf, name: String },
    /// ابحث بجملة عادية
    Ask { path: PathBuf, question: String },
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
        Cmd::Ask { path, question } => {
            let fns = extractor::scan_project(&path)?;
            let hits = search::search(&fns, &question, 10);
            if hits.is_empty() {
                println!("لا توجد نتائج. جرّب كلمات أخرى.");
            }
            for h in hits {
                println!(
                    "{:>5.1}  {}:{}  {}",
                    h.score,
                    h.func.file,
                    h.func.line,
                    h.func.full_name()
                );
            }
            println!("\n⏱ {:?}", start.elapsed());
        }
    }
    Ok(())
}