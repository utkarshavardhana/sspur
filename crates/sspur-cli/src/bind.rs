use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};
use std::path::Path;

const RESERVED: &[&str] = &["log", "some", "ok", "err", "none", "min", "max", "secret", "pii", "untrusted", "guess", "empty_map", "yield", "resume", "main", "extern", "r"];

fn base(t: &str) -> Option<&'static str> {
    Some(match t {
        "long" | "long int" | "signed long" | "signed long int" if cfg!(windows) => "I32",
        "unsigned long" | "unsigned long int" if cfg!(windows) => "U32",
        "char" | "signed char" | "int8_t" => "I8",
        "unsigned char" | "uint8_t" => "U8",
        "short" | "short int" | "signed short" | "signed short int" | "int16_t" => "I16",
        "unsigned short" | "unsigned short int" | "uint16_t" => "U16",
        "int" | "signed" | "signed int" | "int32_t" => "I32",
        "unsigned" | "unsigned int" | "uint32_t" => "U32",
        "long" | "long int" | "signed long" | "long long" | "long long int" | "signed long long" | "int64_t" => "Int",
        "unsigned long" | "unsigned long int" | "unsigned long long" | "unsigned long long int" | "uint64_t" => "U64",
        "float" => "F32",
        "double" => "F64",
        "_Bool" | "bool" => "Bool",
        "void" => "Unit",
        _ => return None,
    })
}

struct Types {
    typedefs: HashMap<String, String>,
}

impl Types {
    fn resolve(&self, name: &str) -> String {
        let mut n = name.trim().to_string();
        let mut seen = 0;
        while let Some(t) = self.typedefs.get(&n) {
            if base(&n).is_some() || seen > 16 {
                break;
            }
            n = t.clone();
            seen += 1;
        }
        n
    }

    fn map(&self, raw: &str, ret: bool) -> Result<String, String> {
        let t = raw.trim();
        let stars = t.matches('*').count();
        let bad = |why: &str| Err(format!("{t} ({why})"));
        if t.contains('(') || t.contains('[') {
            return bad("function pointer or array");
        }
        let core = t.trim_end_matches(['*', ' ']).trim();
        let is_const = core.split_whitespace().any(|w| w == "const");
        let words: Vec<&str> = core.split_whitespace().filter(|w| !matches!(*w, "const" | "volatile" | "restrict")).collect();
        let resolved = self.resolve(&words.join(" "));
        let resolved = resolved.split_whitespace().filter(|w| !matches!(*w, "const" | "volatile")).collect::<Vec<_>>().join(" ");
        let b = base(&resolved);
        match (stars, b) {
            (0, Some("Unit")) if ret => Ok("Unit".into()),
            (0, Some("Unit")) => bad("void parameter"),
            (0, Some(s)) => Ok(s.into()),
            (1, Some("I8")) if resolved.ends_with("char") && ret => Ok("Opt[Str]".into()),
            (1, Some("I8")) if resolved.ends_with("char") => Ok("Str".into()),
            (1, Some(s)) if s != "Unit" && s != "Bool" && is_const && !ret => Ok(format!("List[{s}]")),
            (1, Some(_)) if !is_const => bad("mutable pointer"),
            (0, None) => bad("unsupported type"),
            _ => bad("unsupported pointer"),
        }
    }
}

fn ret_type(fn_type: &str) -> &str {
    let mut depth = 0;
    for (i, c) in fn_type.char_indices().rev() {
        match c {
            ')' => depth += 1,
            '(' => {
                depth -= 1;
                if depth == 0 {
                    return fn_type[..i].trim();
                }
            }
            _ => {}
        }
    }
    fn_type
}

fn ident(name: &str, taken: &HashSet<String>) -> String {
    let ok = name.starts_with(|c: char| c.is_ascii_lowercase()) && !RESERVED.contains(&name) && !sspur_syntax::lexer::KEYWORDS.contains(&name) && name.chars().all(|c| c == '_' || c.is_ascii_alphanumeric());
    if ok && !taken.contains(name) {
        return name.to_string();
    }
    let mut n = format!("c_{}", name.trim_start_matches('_').to_ascii_lowercase());
    while taken.contains(&n) {
        n.push('_');
    }
    n
}

fn param_name(name: &str, i: usize) -> String {
    if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_lowercase()) || sspur_syntax::lexer::KEYWORDS.contains(&name) || name == "r" {
        if name.is_empty() { format!("a{i}") } else { format!("{}_", name.to_ascii_lowercase().trim_start_matches('_')) }
    } else {
        name.to_string()
    }
}

pub fn bind(header: &str, lib: Option<&str>) -> Result<String, String> {
    let cc = sspur_native::cgen::flags::cc();
    let out = std::process::Command::new(&cc).args(["-Xclang", "-ast-dump=json", "-fsyntax-only", "-x", "c"]).arg(header).output().map_err(|e| format!("cannot run {cc}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cc} could not parse {header}: {}", String::from_utf8_lossy(&out.stderr).lines().take(4).collect::<Vec<_>>().join(" | ")));
    }
    let ast: Json = serde_json::from_slice(&out.stdout).map_err(|e| format!("bad AST JSON: {e}"))?;
    let want = Path::new(header).canonicalize().map_err(|e| e.to_string())?;
    let nodes = ast["inner"].as_array().cloned().unwrap_or_default();
    let mut types = Types { typedefs: HashMap::new() };
    for n in &nodes {
        if n["kind"] == "TypedefDecl"
            && let (Some(name), Some(t)) = (n["name"].as_str(), n["type"]["qualType"].as_str())
        {
            let t = n["type"]["desugaredQualType"].as_str().unwrap_or(t);
            types.typedefs.insert(name.to_string(), t.to_string());
        }
    }
    let mut cur: Option<String> = None;
    let mut decls = Vec::new();
    let mut skipped = Vec::new();
    let mut seen = HashSet::new();
    let mut taken = HashSet::new();
    for n in &nodes {
        for loc in [&n["loc"], &n["loc"]["spellingLoc"], &n["loc"]["expansionLoc"]] {
            if let Some(f) = loc["file"].as_str() {
                cur = Some(f.to_string());
            }
        }
        let here = cur.as_ref().and_then(|c| Path::new(c).canonicalize().ok()).is_some_and(|c| c == want);
        if !here || n["kind"] != "FunctionDecl" || n["isImplicit"] == true {
            continue;
        }
        let Some(name) = n["name"].as_str() else { continue };
        if !seen.insert(name.to_string()) {
            continue;
        }
        let ft = n["type"]["qualType"].as_str().unwrap_or("");
        if n["storageClass"] == "static" {
            skipped.push(format!("{name}: static, no linkable symbol"));
            continue;
        }
        if n["variadic"] == true {
            skipped.push(format!("{name}: variadic"));
            continue;
        }
        let mut params = Vec::new();
        let mut why = None;
        let ps: Vec<&Json> = n["inner"].as_array().map(|v| v.iter().filter(|p| p["kind"] == "ParmVarDecl").collect()).unwrap_or_default();
        for (i, p) in ps.iter().enumerate() {
            let pt = p["type"]["qualType"].as_str().unwrap_or("");
            let pn = param_name(p["name"].as_str().unwrap_or(""), i);
            match types.map(pt, false) {
                Ok(t) => params.push(format!("{pn}: {t}")),
                Err(e) => {
                    why = Some(format!("parameter {pn} has type {e}"));
                    break;
                }
            }
        }
        let ret = match types.map(ret_type(ft), true) {
            Ok(t) => t,
            Err(e) => {
                why.get_or_insert(format!("result type {e}"));
                String::new()
            }
        };
        if let Some(w) = why {
            skipped.push(format!("{name}: {w}"));
            continue;
        }
        let sname = ident(name, &taken);
        taken.insert(sname.clone());
        let mut d = format!("extern fn {sname}({})", params.join(", "));
        if ret != "Unit" {
            d.push_str(&format!(" -> {ret}"));
        }
        d.push_str(" ! ffi");
        if let Some(l) = lib {
            d.push_str(&format!(" from {l:?}"));
        }
        if sname != name {
            d.push_str(&format!(" as {name:?}"));
        }
        decls.push(d);
    }
    let file = Path::new(header).file_name().map_or(header.into(), |f| f.to_string_lossy().into_owned());
    let mut s = format!("// generated by sspur bind from {file}\n");
    for d in &decls {
        s.push_str(d);
        s.push('\n');
    }
    if !skipped.is_empty() {
        s.push_str(&format!("// skipped {} declaration(s):\n", skipped.len()));
        for k in &skipped {
            s.push_str(&format!("//   {k}\n"));
        }
    }
    Ok(s)
}
