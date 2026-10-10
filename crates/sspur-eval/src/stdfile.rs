//! File handles: one descriptor, no user-space buffer, the same syscalls as `std_rt.c` section `file`.
use crate::stdlib::os_reason;
use crate::value::Value;
use crate::{trap, Ctrl, Interp, R};
use crate::sys::{errno, fd_close as close, fd_pread as pread, fd_read as read, fd_seek as lseek, fd_write as write, identity};
use std::rc::Rc;

const EINTR: i32 = 4;
const EBADF: i32 = 9;

struct F {
    fd: i32,
    path: String,
    dev: i64,
    ino: i64,
}

fn s(v: &Value) -> R<&str> {
    match v {
        Value::Str(x) => Ok(x),
        v => trap(format!("expected Str, got {v}")),
    }
}

fn int(v: &Value) -> R<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        v => trap(format!("expected Int, got {v}")),
    }
}

fn reason(path: &str, e: i32) -> String {
    if e == EBADF {
        return format!("{path}: not open for this operation");
    }
    format!("{path}: {}", crate::sys::errno_reason(e))
}

fn ok(v: Value) -> Value {
    Value::Res(Ok(Rc::new(v)))
}

fn err(m: String) -> Value {
    Value::Res(Err(Rc::new(Value::str(&m))))
}

fn file_of(v: &Value) -> R<F> {
    let Value::Record(_, fs) = v else { return trap(format!("expected File, got {v}")) };
    Ok(F { fd: int(&fs[0].1)? as i32, path: s(&fs[1].1)?.to_string(), dev: int(&fs[2].1)?, ino: int(&fs[3].1)? })
}

fn live(f: &F) -> Result<(), String> {
    match identity(f.fd) {
        Some((d, i)) if d == f.dev && i == f.ino => Ok(()),
        _ => Err(format!("{}: file is closed", f.path)),
    }
}

pub fn open(path: &str, mode: &str) -> R<Result<Value, String>> {
    let mut o = std::fs::OpenOptions::new();
    match mode {
        "r" => o.read(true),
        "w" => o.write(true).create(true).truncate(true),
        "a" => o.append(true).create(true),
        "r+" => o.read(true).write(true),
        "w+" => o.read(true).write(true).create(true).truncate(true),
        "a+" => o.read(true).append(true).create(true),
        _ => return trap(format!("bad file mode '{mode}'")),
    };
    if path.contains('\0') {
        return Ok(Err(format!("{path}: invalid path")));
    }
    let file = match o.open(path) {
        Ok(f) => f,
        Err(e) => return Ok(Err(format!("{path}: {}", os_reason(&e)))),
    };
    let fd = crate::sys::into_fd(file);
    let (dev, ino) = identity(fd).unwrap_or((0, 0));
    Ok(Ok(Value::Record(
        "#File".into(),
        Rc::new(vec![("fd".into(), Value::Int(fd as i64)), ("path".into(), Value::str(path)), ("dev".into(), Value::Int(dev)), ("ino".into(), Value::Int(ino))]),
    )))
}

fn close_fd(f: &F) -> Result<(), String> {
    live(f)?;
    if unsafe { close(f.fd) } != 0 {
        return Err(reason(&f.path, errno()));
    }
    Ok(())
}

fn read_n(f: &F, n: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    while out.len() < n {
        let want = (n - out.len()).min(buf.len());
        let k = unsafe { read(f.fd, buf.as_mut_ptr().cast(), want) };
        if k < 0 {
            let e = errno();
            if e == EINTR {
                continue;
            }
            return Err(reason(&f.path, e));
        }
        if k == 0 {
            break;
        }
        out.extend_from_slice(&buf[..k as usize]);
    }
    Ok(out)
}

fn line(f: &F) -> Result<Option<String>, String> {
    let mut out = Vec::new();
    let mut got = false;
    let pos = unsafe { lseek(f.fd, 0, 1) };
    if pos >= 0 {
        let mut buf = [0u8; 4096];
        let mut used: i64 = 0;
        loop {
            let k = unsafe { pread(f.fd, buf.as_mut_ptr().cast(), buf.len(), pos + used) };
            if k < 0 {
                let e = errno();
                if e == EINTR {
                    continue;
                }
                return Err(reason(&f.path, e));
            }
            if k == 0 {
                break;
            }
            got = true;
            let chunk = &buf[..k as usize];
            if let Some(j) = chunk.iter().position(|c| *c == b'\n') {
                out.extend_from_slice(&chunk[..j]);
                used += j as i64 + 1;
                break;
            }
            out.extend_from_slice(chunk);
            used += k as i64;
        }
        unsafe { lseek(f.fd, pos + used, 0) };
    } else {
        loop {
            let mut ch = 0u8;
            let k = unsafe { read(f.fd, (&mut ch as *mut u8).cast(), 1) };
            if k < 0 && errno() == EINTR {
                continue;
            }
            if k < 0 {
                return Err(reason(&f.path, errno()));
            }
            if k == 0 {
                break;
            }
            got = true;
            if ch == b'\n' {
                break;
            }
            out.push(ch);
        }
    }
    if !got {
        return Ok(None);
    }
    if out.last() == Some(&b'\r') {
        out.pop();
    }
    String::from_utf8(out).map(Some).map_err(|_| format!("{}: invalid UTF-8", f.path))
}

fn write_all(f: &F, b: &[u8]) -> Result<(), String> {
    let mut off = 0;
    while off < b.len() {
        let k = unsafe { write(f.fd, b[off..].as_ptr().cast(), b.len() - off) };
        if k < 0 {
            let e = errno();
            if e == EINTR {
                continue;
            }
            return Err(reason(&f.path, e));
        }
        off += k as usize;
    }
    Ok(())
}

fn res(r: Result<Value, String>) -> Value {
    r.map_or_else(err, ok)
}

impl Interp {
    pub(crate) fn file_global(&self, n: &str, a: &[Value]) -> R<Option<Value>> {
        Ok(Some(match n {
            "open_file" => res(open(s(&a[0])?, s(&a[1])?)?),
            "with_file" => {
                let h = match open(s(&a[0])?, s(&a[1])?)? {
                    Ok(h) => h,
                    Err(e) => return Ok(Some(err(e))),
                };
                let r = self.apply(&a[2], vec![h.clone()]);
                let f = file_of(&h)?;
                if !matches!(r, Err(Ctrl::Trap(_))) {
                    let _ = close_fd(&f);
                }
                ok(r?)
            }
            _ => return Ok(None),
        }))
    }

    pub(crate) fn file_method(&self, name: &str, recv: &Value, a: &[Value]) -> R {
        let f = file_of(recv)?;
        if name == "read" && int(&a[0])? < 0 {
            return trap("read size must be >= 0");
        }
        if let Err(e) = live(&f) {
            return Ok(err(e));
        }
        Ok(res(match name {
            "read" => read_n(&f, int(&a[0])? as usize).map(|b| Value::list(b.into_iter().map(|x| Value::Int(x as i64)).collect())),
            "read_line" => line(&f).map(|l| Value::Opt(l.map(|x| Rc::new(Value::str(&x))))),
            "read_all" => read_n(&f, usize::MAX).and_then(|b| String::from_utf8(b).map(|x| Value::str(&x)).map_err(|_| format!("{}: invalid UTF-8", f.path))),
            "lines" => {
                let mut out = Vec::new();
                loop {
                    match line(&f) {
                        Ok(Some(l)) => out.push(Value::str(&l)),
                        Ok(None) => break Ok(Value::list(out)),
                        Err(e) => break Err(e),
                    }
                }
            }
            "fold_lines" => {
                let mut acc = a[0].clone();
                loop {
                    match line(&f) {
                        Ok(Some(l)) => acc = self.apply(&a[1], vec![acc, Value::str(&l)])?,
                        Ok(None) => break Ok(acc),
                        Err(e) => break Err(e),
                    }
                }
            }
            "write" => write_all(&f, s(&a[0])?.as_bytes()).map(|_| Value::Unit),
            "write_bytes" => {
                let mut bs = Vec::new();
                for x in match &a[0] {
                    Value::List(xs) => xs.iter(),
                    v => return trap(format!("expected List, got {v}")),
                } {
                    match u8::try_from(int(x)?) {
                        Ok(b) => bs.push(b),
                        Err(_) => return Ok(err(format!("{}: byte out of range", f.path))),
                    }
                }
                write_all(&f, &bs).map(|_| Value::Unit)
            }
            "seek" => {
                let p = int(&a[0])?;
                if unsafe { lseek(f.fd, p, 0) } < 0 { Err(reason(&f.path, errno())) } else { Ok(Value::Unit) }
            }
            "tell" => {
                let p = unsafe { lseek(f.fd, 0, 1) };
                if p < 0 { Err(reason(&f.path, errno())) } else { Ok(Value::Int(p)) }
            }
            "size" => {
                crate::sys::fd_len(f.fd).map(|n| Value::Int(n as i64)).map_err(|e| format!("{}: {}", f.path, os_reason(&e)))
            }
            "close" => close_fd(&f).map(|_| Value::Unit),
            _ => return trap(format!("no method '{name}' on File")),
        }))
    }
}
