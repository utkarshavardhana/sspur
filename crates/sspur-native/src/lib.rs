//! Native code for SSPUR. The value, number, time, locale and JSON modules are plain Rust and
//! build everywhere; the Cranelift JIT and the C and LLVM back ends sit behind the `jit` feature.
pub mod bigint;
pub mod chrono;
pub mod json;
pub mod locale;
pub mod nval;
pub mod rnd;
pub mod tz;

#[cfg(feature = "jit")]
pub mod bare;
#[cfg(feature = "jit")]
pub mod cgen;
#[cfg(feature = "jit")]
pub mod llvm;
#[cfg(feature = "jit")]
mod jit;
#[cfg(feature = "jit")]
pub use jit::*;

pub type LogHook = (fn(*const (), &str), *const ());

thread_local! {
    pub(crate) static LOG_HOOK: std::cell::Cell<Option<LogHook>> = const { std::cell::Cell::new(None) };
}

static PROGRAM_ARGS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

pub fn set_program_args(args: Vec<String>) {
    *PROGRAM_ARGS.lock().unwrap() = args;
}

pub fn program_args() -> Vec<String> {
    PROGRAM_ARGS.lock().unwrap().clone()
}

pub fn set_log_hook(hook: Option<LogHook>) {
    LOG_HOOK.with(|h| h.set(hook));
}
