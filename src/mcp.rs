use crate::{embed, extractor, extractor::Function, search};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

struct Ctx {
    root: PathBuf,
    funcs: Vec<Function>,
    vecs: Vec<Vec<f32>>,
    emb: embed::Embedder,
}

type Reply = Result<Value, (i32, String)>;

pub fn serve(root: &Path) -> anyhow::Result<()> {
    // مهم: كل الرسائل النصية تذهب إلى stderr، فـ stdout مخصص للبروتوكول فقط
    let funcs = extractor::scan_project(root)?;
    let mut emb = embed::Embedder::new()?;
    let vecs = embed::load_or_build(root, &funcs, &mut emb)?;
    eprintln!("codemap MCP جاهز: {} دالة في {}", funcs.len(), root.display());

    let mut ctx = Ctx {
        root: root.to_path_buf(),
        funcs,
        vecs,
        emb,
    };

    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                let r = json!({"jsonrpc":"2.0","id":null,
                    "error":{"code":-32700,"message":"Parse error"}});
                writeln!(out, "{}", r)?;
                out.flush()?;
                continue;
            }
        };

        // الإشعارات (بدون id) لا تُرَدّ عليها
        let Some(id) = msg.get("id").cloned() else {
            continue;
        };
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(Value::Null);

        let response = match handle(method, &params, &mut ctx) {
            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
            Err((code, message)) => {
                json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
            }
        };
        writeln!(out, "{}", response)?;
        out.flush()?;
    }
    Ok(())
}

fn handle(method: &str, params: &Value, ctx: &mut Ctx) -> Reply {
    match method {
        "initialize" => {
            let version = params
                .get("protocolVersion")
                .and_then(|v| v.as_str())
                .unwrap_or("2024-11-05");
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "codemap", "version": env!("CARGO_PKG_VERSION") }
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools_list()),
        "tools/call" => {
            let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            Ok(match call_tool(name, &args, ctx) {
                Ok(text) => json!({"content":[{"type":"text","text":text}],"isError":false}),
                Err(e) => json!({"content":[{"type":"text","text":e}],"isError":true}),
            })
        }
        _ => Err((-32601, format!("Method not found: {}", method))),
    }
}

fn tools_list() -> Value {
    json!({ "tools": [
        {
            "name": "search_code",
            "description": "Search the codebase by meaning. Describe what the code does in plain English (e.g. 'stop a running task') and get the most relevant functions with file and line.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "question": { "type": "string", "description": "What you are looking for, in plain English" },
                    "limit": { "type": "integer", "description": "Number of results (default 5, max 20)" },
                    "mode": { "type": "string", "enum": ["semantic", "hybrid", "lexical"], "description": "Default: semantic" }
                },
                "required": ["question"]
            }
        },
        {
            "name": "find_callers",
            "description": "List the functions that call a function with the given name. Matches by name only, so common names like 'new' return many results.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Function name, e.g. 'poll_acquire'" }
                },
                "required": ["name"]
            }
        }
    ]})
}

fn rel(root: &Path, file: &str) -> String {
    Path::new(file)
        .strip_prefix(root)
        .unwrap_or(Path::new(file))
        .display()
        .to_string()
}

fn call_tool(name: &str, args: &Value, ctx: &mut Ctx) -> Result<String, String> {
    match name {
        "search_code" => {
            let question = args
                .get("question")
                .and_then(|q| q.as_str())
                .ok_or("missing 'question'")?
                .to_string();
            let limit = args
                .get("limit")
                .and_then(|l| l.as_u64())
                .unwrap_or(5)
                .clamp(1, 20) as usize;
            let mode = args
                .get("mode")
                .and_then(|m| m.as_str())
                .unwrap_or("semantic");

            let hits = if mode == "lexical" {
                search::search(&ctx.funcs, &question, limit)
            } else {
                let q = ctx
                    .emb
                    .embed(vec![question.clone()])
                    .map_err(|e| e.to_string())?
                    .remove(0);
                if mode == "hybrid" {
                    search::hybrid_search(&ctx.funcs, &ctx.vecs, &question, &q, limit)
                } else {
                    search::semantic_search(&ctx.funcs, &ctx.vecs, &q, limit)
                }
            };

            if hits.is_empty() {
                return Ok("No results. Try different words.".to_string());
            }
            let mut text = String::new();
            for (i, h) in hits.iter().enumerate() {
                let f = h.func;
                text.push_str(&format!(
                    "{}. {}:{}  {}\n",
                    i + 1,
                    rel(&ctx.root, &f.file),
                    f.line,
                    f.full_name()
                ));
                if !f.doc.is_empty() {
                    let d: String = f.doc.chars().take(160).collect();
                    text.push_str(&format!("   {}\n", d));
                }
            }
            Ok(text)
        }
        "find_callers" => {
            let fname = args
                .get("name")
                .and_then(|n| n.as_str())
                .ok_or("missing 'name'")?;
            let mut text = String::new();
            let mut n = 0;
            for f in ctx.funcs.iter().filter(|f| f.calls.iter().any(|c| c == fname)) {
                text.push_str(&format!(
                    "{}:{}  {}\n",
                    rel(&ctx.root, &f.file),
                    f.line,
                    f.full_name()
                ));
                n += 1;
            }
            if n == 0 {
                return Ok(format!(
                    "No function calls `{}`. It may be a file name or a misspelling.",
                    fname
                ));
            }
            Ok(format!("{} callers of `{}`:\n{}", n, fname, text))
        }
        _ => Err(format!("Unknown tool: {}", name)),
    }
}