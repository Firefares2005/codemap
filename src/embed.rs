use crate::extractor::Function;
use anyhow::Result;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub struct Embedder {
    model: TextEmbedding,
}

/// مجلد تخزين النموذج (يُحمَّل مرة واحدة فقط)
fn model_cache_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".codemap_models")
}

impl Embedder {
    pub fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_cache_dir(model_cache_dir())
                .with_show_download_progress(true),
        )?;
        Ok(Self { model })
    }

    pub fn embed(&mut self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        Ok(self.model.embed(texts, None)?)
    }
}

/// النص الذي يمثل الدالة: النوع + الاسم + التوثيق + بداية النص
fn doc_text(f: &Function) -> String {
    let owner = f.owner.clone().unwrap_or_default();
    let name = f.name.replace('_', " ");
    let body: String = f.body.chars().take(400).collect();
    format!("{} {}. {}\n{}", owner, name, f.doc, body)
}

/// بصمة المشروع: إذا تغيّر أي كود تتغير البصمة ويُعاد بناء الفهرس
fn fingerprint(funcs: &[Function]) -> u64 {
    let mut h = DefaultHasher::new();
    for f in funcs {
        f.file.hash(&mut h);
        f.line.hash(&mut h);
        f.body.hash(&mut h);
        f.doc.hash(&mut h);
    }
    h.finish()
}

fn cache_path(root: &Path) -> PathBuf {
    root.join(".codemap").join("embeddings.bin")
}

fn load(path: &Path, fp: u64, n: usize) -> Option<Vec<Vec<f32>>> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 24 {
        return None;
    }
    let rd = |i: usize| u64::from_le_bytes(data[i..i + 8].try_into().unwrap());
    if rd(0) != fp || rd(8) as usize != n {
        return None;
    }
    let dim = rd(16) as usize;
    let floats = &data[24..];
    if floats.len() != n * dim * 4 {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut v = Vec::with_capacity(dim);
        for j in 0..dim {
            let o = (i * dim + j) * 4;
            v.push(f32::from_le_bytes(floats[o..o + 4].try_into().unwrap()));
        }
        out.push(v);
    }
    Some(out)
}

fn save(path: &Path, fp: u64, vecs: &[Vec<f32>]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let dim = vecs.first().map(|v| v.len()).unwrap_or(0);
    let mut buf = Vec::with_capacity(24 + vecs.len() * dim * 4);
    buf.extend_from_slice(&fp.to_le_bytes());
    buf.extend_from_slice(&(vecs.len() as u64).to_le_bytes());
    buf.extend_from_slice(&(dim as u64).to_le_bytes());
    for v in vecs {
        for x in v {
            buf.extend_from_slice(&x.to_le_bytes());
        }
    }
    std::fs::write(path, buf)?;
    Ok(())
}

/// يحمّل الفهرس المحفوظ إن كان مطابقاً، وإلا يبنيه ويحفظه
pub fn load_or_build(
    root: &Path,
    funcs: &[Function],
    embedder: &mut Embedder,
) -> Result<Vec<Vec<f32>>> {
    let fp = fingerprint(funcs);
    let path = cache_path(root);

    if let Some(v) = load(&path, fp, funcs.len()) {
        eprintln!("📦 استُخدم الفهرس المحفوظ ({} دالة)", funcs.len());
        return Ok(v);
    }

    eprintln!("🧠 بناء embeddings لـ {} دالة (مرة واحدة فقط)...", funcs.len());
    let texts: Vec<String> = funcs.iter().map(doc_text).collect();
    let vecs = embedder.embed(texts)?;
    save(&path, fp, &vecs)?;
    Ok(vecs)
}