mod extractor;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
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
    }
    Ok(())
}