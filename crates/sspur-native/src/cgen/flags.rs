use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lto {
    #[default]
    Off,
    Thin,
    Full,
}

impl Lto {
    pub fn parse(s: &str) -> Option<Lto> {
        match s {
            "off" | "0" | "none" => Some(Lto::Off),
            "thin" | "1" => Some(Lto::Thin),
            "full" | "fat" => Some(Lto::Full),
            _ => None,
        }
    }

    pub fn flag(self) -> Option<&'static str> {
        match self {
            Lto::Off => None,
            Lto::Thin => Some("-flto=thin"),
            Lto::Full => Some("-flto=full"),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Pgo {
    #[default]
    Off,
    Generate(PathBuf),
    Use(PathBuf),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildOpts {
    pub lto: Lto,
    pub pgo: Pgo,
}

static OPTS: Mutex<Option<BuildOpts>> = Mutex::new(None);

pub fn set(o: BuildOpts) {
    *OPTS.lock().unwrap() = Some(o);
}

pub fn current() -> BuildOpts {
    if let Some(o) = OPTS.lock().unwrap().clone() {
        return o;
    }
    let lto = std::env::var("SSPUR_LTO").ok().and_then(|v| Lto::parse(&v)).unwrap_or_default();
    let pgo = match std::env::var_os("SSPUR_PGO_GEN") {
        Some(d) => Pgo::Generate(PathBuf::from(d)),
        None => Pgo::Off,
    };
    BuildOpts { lto, pgo }
}

pub fn pgo_paths(key_text: &str, opt: &str) -> (PathBuf, PathBuf) {
    let k = blake3::hash(format!("{opt}\n{key_text}").as_bytes()).to_hex()[..32].to_string();
    let d = super::cache_dir().join("pgo");
    (d.join(format!("{k}.raw")), d.join(format!("{k}.profdata")))
}

fn cc() -> String {
    std::env::var("CC").unwrap_or_else(|_| "clang".into())
}

fn on_path(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.components().count() > 1 {
        return p.exists().then(|| p.to_path_buf());
    }
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|c| c.exists())
}

pub fn profdata_tool() -> Result<Vec<String>, String> {
    if let Ok(t) = std::env::var("SSPUR_PROFDATA") {
        return Ok(vec![t]);
    }
    if let Some(c) = on_path(&cc()) {
        let real = std::fs::canonicalize(&c).unwrap_or(c);
        if let Some(t) = real.parent().map(|d| d.join("llvm-profdata")).filter(|t| t.exists()) {
            return Ok(vec![t.display().to_string()]);
        }
        if cfg!(target_os = "macos") && real.starts_with("/usr/bin") {
            return Ok(vec!["xcrun".into(), "llvm-profdata".into()]);
        }
    }
    on_path("llvm-profdata").map(|p| vec![p.display().to_string()]).ok_or_else(|| "llvm-profdata not found (set SSPUR_PROFDATA)".into())
}

pub fn merge(raw: &Path, out: &Path) -> Result<usize, String> {
    let files: Vec<PathBuf> = std::fs::read_dir(raw).map_err(|e| format!("no training profile in {}: {e}", raw.display()))?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "profraw")).collect();
    if files.is_empty() {
        return Err(format!("the training run wrote no profile into {}", raw.display()));
    }
    let tool = profdata_tool()?;
    let o = Command::new(&tool[0]).args(&tool[1..]).arg("merge").arg("-o").arg(out).args(&files).output().map_err(|e| format!("cannot run {}: {e}", tool.join(" ")))?;
    if !o.status.success() {
        return Err(format!("llvm-profdata merge failed: {}", String::from_utf8_lossy(&o.stderr).lines().take(4).collect::<Vec<_>>().join(" | ")));
    }
    Ok(files.len())
}

pub(super) fn build_pgo(src: &str, opt: &str, links: &[String], o: &BuildOpts) -> Result<PathBuf, String> {
    let links = super::resolve_links(links)?;
    let cc = cc();
    let (flag, tag) = match &o.pgo {
        Pgo::Generate(d) => (format!("-fprofile-generate={}", d.display()), d.display().to_string()),
        Pgo::Use(p) => {
            let data = std::fs::read(p).map_err(|e| format!("cannot read profile {}: {e}", p.display()))?;
            (format!("-fprofile-use={}", p.display()), blake3::hash(&data).to_hex().to_string())
        }
        Pgo::Off => return Err("no PGO mode".into()),
    };
    let lto = o.lto.flag();
    let key = blake3::hash(format!("{cc} {opt} {flag} {tag} {lto:?}{}\n{src}", links.iter().map(|l| format!(" {l}")).collect::<String>()).as_bytes()).to_hex()[..32].to_string();
    let base = super::cache_dir();
    let lib = base.join(format!("{key}.{}", std::env::consts::DLL_EXTENSION));
    if lib.exists() {
        return Ok(lib);
    }
    let dir = base.join("pgo").join(&key);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("prog.c"), src).map_err(|e| e.to_string())?;
    let tmp = lib.with_extension(format!("{}.tmp", std::process::id()));
    let mut cmd = Command::new(&cc);
    cmd.current_dir(&dir).args([opt, "-shared", "-fPIC", "-w", &flag]);
    if let Some(l) = lto {
        cmd.arg(l);
    }
    let out = cmd.arg("-o").arg(&tmp).arg("prog.c").args(&links).output().map_err(|e| format!("cannot run {cc}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cc} failed: {}", String::from_utf8_lossy(&out.stderr).lines().take(6).collect::<Vec<_>>().join(" | ")));
    }
    std::fs::rename(&tmp, &lib).map_err(|e| e.to_string())?;
    Ok(lib)
}
