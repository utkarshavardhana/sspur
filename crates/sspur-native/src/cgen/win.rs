//! Windows: the generated C keeps its POSIX shape and `adapt` swaps in Win32 equivalents, so other targets get byte-identical C.

pub(super) const WIN_RT: &str = include_str!("win_rt.c");

#[cfg(test)]
thread_local! {
    pub(super) static FORCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn on() -> bool {
    #[cfg(test)]
    if FORCE.with(|f| f.get()) {
        return true;
    }
    cfg!(windows) || std::env::var_os("SSPUR_C_TARGET").is_some_and(|v| v == "windows")
}

pub(super) fn section(key: &str) -> Option<(Vec<&'static str>, &'static str)> {
    WIN_RT.split("//@ ").skip(1).find_map(|part| {
        let (head, body) = part.split_once('\n').unwrap_or((part, ""));
        let mut words = head.split_whitespace();
        (words.next() == Some(key)).then(|| (words.collect(), body))
    })
}

fn body(key: &str) -> &'static str {
    section(key).map(|(_, b)| b).unwrap_or_else(|| panic!("missing windows runtime section {key}"))
}

const EDITS: &[(&str, &str)] = &[
    ("#include <pthread.h>\n#include <unistd.h>\n", "threads"),
    ("#include <sys/mman.h>\n", "mman"),
    ("static void gc_release_page(size_t i) {", "commit"),
];

pub fn adapt(src: &str) -> String {
    let mut s = src.to_string();
    for (from, key) in EDITS {
        let to = if *key == "commit" { format!("{}{from}", body(key)) } else { body(key).to_string() };
        s = s.replacen(from, &to, 1);
    }
    s = s.replacen("static size_t gc_take_pages(size_t n) {", "static size_t gc_take_pages0(size_t n) {", 1);
    s = s.replacen("static __attribute__((destructor)) void par_fini(void) {", "static void par_fini(void) {", 1);
    let mut out = String::with_capacity(s.len() + 4096);
    for line in s.split_inclusive('\n') {
        if line.starts_with("int64_t sspur_") || line.starts_with("void sspur_") || line.starts_with("SspurDb sspur_db;") {
            out.push_str("__declspec(dllexport) ");
        }
        out.push_str(line);
    }
    out
}
