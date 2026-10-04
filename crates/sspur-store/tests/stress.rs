//! Exit criterion of Phase 7: many concurrent agents on one codebase, no lost work.
//! `SSPUR_STRESS_AGENTS` (default 100) and `SSPUR_STRESS_EDITS` (default 8) size the run.

use serde_json::{json, Value as Json};
use sspur_store::{load_src, Store, Tx, TxResult};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

const SHARED: usize = 5;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

#[derive(Default)]
struct Tally {
    txs: usize,
    acked: usize,
    conflicts: usize,
    rebased: usize,
    merged: usize,
    unexplained: usize,
}

struct Own {
    name: String,
    c: u64,
    alive: bool,
}

struct Report {
    owns: Vec<Own>,
    tests: Vec<String>,
    shared: Vec<(String, usize, u64)>,
    acked_commits: Vec<String>,
    tally: Tally,
}

fn env(k: &str, d: usize) -> usize {
    std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d)
}

fn hub_name(s: &Store, hub_id: &str) -> String {
    let r = s.head_root().unwrap();
    r.ids.iter().find(|(_, i)| i.id == hub_id).map(|(n, _)| n.clone()).unwrap()
}

fn send(s: &Store, agent: &str, ops: Json) -> TxResult {
    let t: Tx = serde_json::from_value(json!({"agent": agent, "reason": "stress", "ops": ops})).unwrap();
    s.apply(t)
}

fn agent(dir: PathBuf, i: usize, edits: usize, hub_id: String) -> Report {
    let s = Store::find(&dir).unwrap();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ ((i as u64 + 1) * 0x1000_0001));
    let me = format!("a{i}");
    let mut rep = Report { owns: vec![], tests: vec![], shared: vec![], acked_commits: vec![], tally: Tally::default() };
    let mut k = 0;
    for _ in 0..edits {
        let hub = hub_name(&s, &hub_id);
        let roll = rng.below(100);
        let live: Vec<usize> = (0..rep.owns.len()).filter(|j| rep.owns[*j].alive).collect();
        let (ops, what): (Json, &str) = if i == 0 && roll < 30 {
            (json!([{"op": "rename", "from": hub, "to": format!("hub_v{}", rng.below(1_000_000))}]), "hub")
        } else if live.is_empty() || roll < 25 {
            let c = rng.below(1000);
            let name = format!("{me}_f{k}");
            k += 1;
            rep.owns.push(Own { name: name.clone(), c, alive: false });
            (json!([{"op": "add", "path": name, "src": format!("fn {name}(x: Int) -> Int\n= x + {c} + {hub}(0)")}]), "add")
        } else if roll < 45 {
            let j = live[rng.below(live.len() as u64) as usize];
            let c = rng.below(1000);
            let name = rep.owns[j].name.clone();
            rep.owns[j].c = c;
            (json!([{"op": "replace", "path": name, "src": format!("fn {name}(x: Int) -> Int\n= x + {c} + {hub}(0)")}]), "change")
        } else if roll < 60 {
            let j = live[rng.below(live.len() as u64) as usize];
            let to = format!("{}_r{}", rep.owns[j].name.split("_r").next().unwrap(), rng.below(1_000_000));
            let from = std::mem::replace(&mut rep.owns[j].name, to.clone());
            (json!([{"op": "rename", "from": from, "to": to}]), "rename")
        } else if roll < 78 {
            let j = live[rng.below(live.len() as u64) as usize];
            let name = &rep.owns[j].name;
            (json!([{"op": "attach", "target": name, "kind": "test", "value": format!("{name}(1) - {name}(0) == 1")}]), "test")
        } else {
            let j = rng.below(SHARED as u64) as usize;
            let c = rng.below(1000);
            (json!([{"op": "replace", "path": format!("shared_{j}"), "src": format!("fn shared_{j}(x: Int) -> Int\n= x + {c}")}]), "shared")
        };
        rep.tally.txs += 1;
        let mut r = send(&s, &me, ops.clone());
        if !r.ok && !r.conflicts.is_empty() {
            rep.tally.conflicts += 1;
            if what == "shared" || rng.below(2) == 0 {
                let fix: Vec<Json> = r.diags.iter().flat_map(|d| d.fix.clone()).collect();
                let retry = if fix.is_empty() { ops.clone() } else { json!(fix) };
                rep.tally.rebased += 1;
                r = send(&s, &me, retry);
            }
        }
        if !r.ok && r.conflicts.is_empty() && r.diags.iter().any(|d| d.code == "E_MERGE" || d.code == "E_OP_MISSING") {
            rep.tally.merged += 1;
            r = send(&s, &me, ops.clone());
        }
        let last = rep.owns.len().saturating_sub(1);
        if r.ok {
            rep.tally.acked += 1;
            if let Some(c) = &r.commit {
                rep.acked_commits.push(c.clone());
            }
            match what {
                "add" => rep.owns[last].alive = true,
                "test" => {
                    for c in &r.changes {
                        if c.old.is_none() && c.renamed_from.is_none() && c.path.contains("_t") {
                            rep.tests.push(c.path.clone());
                        }
                    }
                }
                "shared" => {
                    let c = ops[0]["src"].as_str().unwrap_or_default();
                    let j: usize = c.split("shared_").nth(1).and_then(|t| t.split('(').next()).and_then(|t| t.parse().ok()).unwrap_or(0);
                    let k: u64 = c.rsplit("x + ").next().and_then(|t| t.trim().parse().ok()).unwrap_or(0);
                    rep.shared.push((r.commit.clone().unwrap_or_default(), j, k));
                }
                _ => {}
            }
        } else {
            if r.conflicts.is_empty() && !r.diags.iter().any(|d| d.code == "E_MERGE") {
                rep.tally.unexplained += 1;
                eprintln!("{me}: {what} rejected: {:?}", r.diags.first());
            }
            match what {
                "add" => {
                    rep.owns.pop();
                }
                "change" | "rename" => panic!("agent {me}: edit of its own definition rejected: {:?}", r.diags),
                _ => {}
            }
        }
    }
    rep
}

#[test]
fn hundred_agents_one_codebase_no_lost_work() {
    let n = env("SSPUR_STRESS_AGENTS", 100);
    let edits = env("SSPUR_STRESS_EDITS", 8);
    let dir = std::env::temp_dir().join(format!("sspur-stress-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::init(&dir).unwrap();
    let mut ops = vec![json!({"op": "add", "path": "hub", "src": "fn hub(x: Int) -> Int\n= 0"})];
    for j in 0..SHARED {
        ops.push(json!({"op": "add", "path": format!("shared_{j}"), "src": format!("fn shared_{j}(x: Int) -> Int\n= x + 0")}));
        ops.push(json!({"op": "attach", "target": format!("shared_{j}"), "kind": "test", "value": format!("shared_{j}(1) - shared_{j}(0) == 1")}));
    }
    let r = send(&s, "seed", json!(ops));
    assert!(r.ok, "{:?}", r.diags);
    let hub_id = s.head_root().unwrap().ids["hub"].id.clone();
    let t0 = Instant::now();
    let reports = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for i in 0..n {
            let (dir, hub_id, reports) = (dir.clone(), hub_id.clone(), &reports);
            std::thread::Builder::new()
                .stack_size(32 << 20)
                .spawn_scoped(sc, move || {
                    let rep = agent(dir, i, edits, hub_id);
                    reports.lock().unwrap().push((i, rep));
                })
                .unwrap();
        }
    });
    let elapsed = t0.elapsed();
    let reports = reports.into_inner().unwrap();
    assert_eq!(reports.len(), n, "every agent finished");

    let fin = load_src(s.load_head().unwrap().src).unwrap();
    assert!(!fin.check.has_errors(), "{:?}", fin.check.diags.iter().filter(|d| d.is_error()).take(5).collect::<Vec<_>>());
    let mut it = sspur_eval::Interp::new(&fin.module, fin.check.record_types.clone(), fin.check.user_methods.clone(), fin.check.gen_loops.clone());
    it.set_ownership(fin.check.own.moves.clone(), fin.check.own.inplace.clone());
    it.set_check(&fin.check);
    let results = it.run_tests();
    let failed: Vec<_> = results.iter().filter(|(_, r)| r.is_err()).collect();
    assert!(failed.is_empty(), "{failed:?}");

    let ix = s.index().unwrap();
    let ancestry = s.ancestors(&ix.heads);
    let mut chain: Vec<String> = Vec::new();
    let mut cur = ix.heads.first().cloned();
    while let Some(c) = cur {
        let cm = s.commit(&c).unwrap();
        cur = cm.parents.first().cloned();
        chain.push(c);
    }
    chain.reverse();
    let order: HashMap<&str, usize> = chain.iter().enumerate().map(|(i, c)| (c.as_str(), i)).collect();
    let bodies: BTreeMap<String, String> = fin.module.defs.iter().map(|d| (d.name().to_string(), sspur_syntax::printer::print_def(d))).collect();
    let hub = hub_name(&s, &hub_id);
    let mut lost = Vec::new();
    let mut total = Tally::default();
    let mut last_shared: HashMap<usize, (usize, u64)> = HashMap::new();
    let mut tests = 0;
    let mut owned = 0;
    for (i, rep) in &reports {
        for c in &rep.acked_commits {
            if !ancestry.contains(c) {
                lost.push(format!("a{i}: acked commit {c} is not in the final history"));
            }
        }
        for o in rep.owns.iter().filter(|o| o.alive) {
            owned += 1;
            let want = format!("x + {} + {hub}(0)", o.c);
            match bodies.get(&o.name) {
                Some(b) if b.contains(&want) => {}
                Some(b) => lost.push(format!("a{i}: {} is {b:?}, wanted {want}", o.name)),
                None => lost.push(format!("a{i}: {} is missing", o.name)),
            }
        }
        for t in &rep.tests {
            tests += 1;
            if !bodies.contains_key(t) {
                lost.push(format!("a{i}: test {t} is missing"));
            }
        }
        for (c, j, k) in &rep.shared {
            let Some(&pos) = order.get(c.as_str()) else { continue };
            if last_shared.get(j).is_none_or(|(p, _)| *p < pos) {
                last_shared.insert(*j, (pos, *k));
            }
        }
        total.txs += rep.tally.txs;
        total.acked += rep.tally.acked;
        total.conflicts += rep.tally.conflicts;
        total.rebased += rep.tally.rebased;
        total.merged += rep.tally.merged;
        total.unexplained += rep.tally.unexplained;
    }
    for (j, (_, k)) in &last_shared {
        let b = &bodies[&format!("shared_{j}")];
        if !b.contains(&format!("x + {k}")) {
            lost.push(format!("shared_{j} is {b:?}, the last acknowledged write set {k}"));
        }
    }
    let seen: HashSet<&String> = reports.iter().flat_map(|(_, r)| r.acked_commits.iter()).collect();
    eprintln!(
        "stress: {n} agents, {} transactions, {} acknowledged ({} distinct commits), {} conflicts reported ({} rebased), {} merge retries, {owned} owned fns, {tests} tests, {} tests passed, {} definitions, {} rejected without a conflict report, lost {} in {:.1}s",
        total.txs, total.acked, seen.len(), total.conflicts, total.rebased, total.merged, results.len(), fin.module.defs.len(), total.unexplained, lost.len(), elapsed.as_secs_f64()
    );
    assert_eq!(total.unexplained, 0);
    assert!(lost.is_empty(), "lost work: {:?}", &lost[..lost.len().min(10)]);
    assert!(total.acked > 0 && owned > 0);
    let _ = std::fs::remove_dir_all(&dir);
}
