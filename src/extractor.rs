use std::collections::BTreeSet;
use std::path::Path;
use tree_sitter::{Node, Parser};
use walkdir::WalkDir;

pub struct Function {
    pub name: String,
    pub owner: Option<String>, // النوع: impl Framed -> "Framed"
    pub file: String,
    pub line: usize,
    pub body: String,
    pub calls: Vec<String>,
}

impl Function {
    pub fn full_name(&self) -> String {
        match &self.owner {
            Some(o) => format!("{}::{}", o, self.name),
            None => self.name.clone(),
        }
    }
}

fn is_ignored(path: &Path) -> bool {
    path.components().any(|c| {
        matches!(
            c.as_os_str().to_str(),
            Some("target") | Some("tests") | Some("benches") | Some(".git")
        )
    })
}

pub fn scan_project(root: &Path) -> anyhow::Result<Vec<Function>> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;

    let mut result = Vec::new();

    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") || is_ignored(path) {
            continue;
        }
        // تجاهل الملفات غير القابلة للقراءة بدل إيقاف البرنامج
        let Ok(code) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Some(tree) = parser.parse(&code, None) {
            collect(tree.root_node(), &code, path, None, &mut result);
        }
    }
    Ok(result)
}

fn collect(node: Node, code: &str, path: &Path, owner: Option<&str>, out: &mut Vec<Function>) {
    // عند دخول impl أو trait نسجّل اسم النوع
    let mut current_owner = owner.map(|s| s.to_string());
    if node.kind() == "impl_item" || node.kind() == "trait_item" {
        let field = if node.kind() == "impl_item" { "type" } else { "name" };
        if let Some(t) = node.child_by_field_name(field) {
            current_owner = Some(code[t.byte_range()].to_string());
        }
    }

    if node.kind() == "function_item" {
        if let Some(name) = node.child_by_field_name("name") {
            let mut calls = BTreeSet::new();
            if let Some(body) = node.child_by_field_name("body") {
                find_calls(body, code, &mut calls);
            }
            out.push(Function {
                name: code[name.byte_range()].to_string(),
                owner: current_owner.clone(),
                file: path.display().to_string(),
                line: node.start_position().row + 1,
                body: code[node.byte_range()].to_string(),
                calls: calls.into_iter().collect(),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, code, path, current_owner.as_deref(), out);
    }
}

fn find_calls(node: Node, code: &str, out: &mut BTreeSet<String>) {
    if node.kind() == "call_expression" {
        if let Some(f) = node.child_by_field_name("function") {
            let name = match f.kind() {
                // foo(...)
                "identifier" => Some(code[f.byte_range()].to_string()),
                // Type::foo(...)
                "scoped_identifier" => f
                    .child_by_field_name("name")
                    .map(|n| code[n.byte_range()].to_string()),
                // obj.method(...)
                "field_expression" => f
                    .child_by_field_name("field")
                    .map(|n| code[n.byte_range()].to_string()),
                _ => None,
            };
            if let Some(n) = name {
                out.insert(n);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        find_calls(child, code, out);
    }
}