use crate::schema::diff;
use crate::{iam, Service, Store};
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Same,
    New,
    Removed,
    Compatible,
    Migration,
    Breaking,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Same => "unchanged",
            Kind::New => "new",
            Kind::Removed => "removed",
            Kind::Compatible => "compatible",
            Kind::Migration => "migration",
            Kind::Breaking => "breaking",
        }
    }
}

#[derive(Debug)]
pub struct Change {
    pub store: String,
    pub kind: Kind,
    pub from: Option<String>,
    pub to: Option<String>,
    pub changes: Vec<(bool, String)>,
    pub fun: Option<String>,
    pub reverse: Option<String>,
    pub problems: Vec<String>,
    pub notes: Vec<String>,
    pub side_by_side: bool,
}

pub struct Report {
    pub stores: Vec<Change>,
}

pub fn compare(old: &[Store], new: &[Store]) -> Report {
    let mut out = Vec::new();
    for s in new {
        let mut c = Change { store: s.name.clone(), kind: Kind::Same, from: None, to: Some(s.sv.clone()), changes: vec![], fun: None, reverse: None, problems: vec![], notes: vec![], side_by_side: true };
        let Some(o) = old.iter().find(|o| o.name == s.name) else {
            c.kind = Kind::New;
            out.push(c);
            continue;
        };
        c.from = Some(o.sv.clone());
        if o.key_shape != s.key_shape {
            c.kind = Kind::Breaking;
            c.problems.push(format!("E_MIGRATE_KEY key type changed from {} to {}; keys cannot be migrated in place (add a new store and copy into it)", o.key_shape, s.key_shape));
            out.push(c);
            continue;
        }
        if o.sv == s.sv {
            out.push(c);
            continue;
        }
        diff(&o.shape, &s.shape, "", &mut c.changes);
        let mig = s.migs.iter().find(|g| g.sv == o.sv).or_else(|| s.migs.iter().find(|g| g.shape == o.shape));
        if let Some(g) = mig {
            c.kind = Kind::Migration;
            c.fun = Some(g.fun.clone());
            c.reverse = s.backs.iter().find(|b| b.sv == g.sv).map(|b| b.fun.clone());
            if g.sv != o.sv {
                c.notes.push(format!("{} reads schema {} with the same JSON shape as the stored {}; items tagged {} use it too", g.fun, g.sv, o.sv, o.sv));
            }
            c.side_by_side = c.reverse.is_some();
            match &c.reverse {
                Some(r) => c.notes.push(format!("writes keep a v_{} copy made by {r}, so the old version reads new items", o.sv)),
                None => c.notes.push(format!("the old version cannot read items the new version writes; add `fn unmigrate_{}(new: {}) -> OldT` for a side-by-side rollout", s.name, s.val)),
            }
        } else if c.changes.iter().all(|x| x.0) {
            c.kind = Kind::Compatible;
            if c.changes.iter().any(|x| x.1.contains("added (optional)")) {
                c.notes.push("old items read the added optional fields as none; while both versions run, a write by the old version drops them".into());
            }
            if c.changes.iter().any(|x| x.1.starts_with("variant") || x.1.contains(" variant ")) {
                c.side_by_side = false;
                c.notes.push(format!("the old version cannot read an item holding an added variant; add migrate_{0} and unmigrate_{0} to run side by side", s.name));
            }
        } else {
            c.kind = Kind::Breaking;
            let near: Vec<String> = s.migs.iter().map(|g| format!("{} reads schema {} ({}), not {}", g.fun, g.sv, g.shape, o.sv)).collect();
            c.problems.push(format!("E_MIGRATE_MISSING stored values of {} need `fn migrate_{}(old: OldT) -> {}` where OldT is the previous type {}{}", s.name, s.name, s.val, o.shape, if near.is_empty() { String::new() } else { format!("; {}", near.join("; ")) }));
        }
        out.push(c);
    }
    for o in old.iter().filter(|o| !new.iter().any(|s| s.name == o.name)) {
        out.push(Change { store: o.name.clone(), kind: Kind::Removed, from: Some(o.sv.clone()), to: None, changes: vec![], fun: None, reverse: None, problems: vec![], notes: vec!["the table is retained (DeletionPolicy Retain); nothing is deleted".into()], side_by_side: true });
    }
    Report { stores: out }
}

impl Report {
    pub fn ok(&self) -> bool {
        self.stores.iter().all(|c| c.kind != Kind::Breaking)
    }

    pub fn side_by_side(&self) -> bool {
        self.stores.iter().all(|c| c.side_by_side)
    }

    pub fn json(&self) -> Value {
        let stores: Vec<Value> = self
            .stores
            .iter()
            .map(|c| {
                json!({
                    "store": c.store, "kind": c.kind.name(), "from": c.from, "to": c.to,
                    "changes": c.changes.iter().map(|(ok, d)| json!({"compatible": ok, "change": d})).collect::<Vec<_>>(),
                    "migrate": c.fun, "reverse": c.reverse, "problems": c.problems, "notes": c.notes
                })
            })
            .collect();
        json!({"ok": self.ok(), "side_by_side": self.side_by_side(), "stores": stores})
    }

    pub fn text(&self) -> String {
        let mut s = String::new();
        for c in &self.stores {
            let ver = match (&c.from, &c.to) {
                (Some(a), Some(b)) if a != b => format!("schema {a} -> {b}"),
                (_, Some(b)) | (Some(b), None) => format!("schema {b}"),
                _ => String::new(),
            };
            let via = match (&c.fun, &c.reverse) {
                (Some(f), Some(r)) => format!(" via {f}, reverse {r}"),
                (Some(f), None) => format!(" via {f}"),
                _ => String::new(),
            };
            s.push_str(&format!("  {} {} {ver}{via}\n", c.store, c.kind.name()));
            for (ok, d) in &c.changes {
                s.push_str(&format!("    {} {d}\n", if *ok { "+" } else { "!" }));
            }
            for p in &c.problems {
                s.push_str(&format!("    error {p}\n"));
            }
            for n in &c.notes {
                s.push_str(&format!("    note: {n}\n"));
            }
        }
        s
    }
}

pub fn plan_json(old: &Service, new: &Service, r: &Report) -> Value {
    let backfills: Vec<Value> = new
        .backfills
        .iter()
        .filter(|b| r.stores.iter().any(|c| c.kind == Kind::Migration && b.name == format!("backfill_{}", c.store)))
        .map(|b| json!({"function": crate::pascal(&b.name), "store": b.name.trim_start_matches("backfill_"), "actions": iam::actions(b)}))
        .collect();
    let phases = vec![
        json!({"phase": "expand", "do": "deploy.sh: CodeDeploy shifts each alias to the new version (canary, then all); new handlers read old items through the migration (lazy, in memory) and write the new schema", "rollback": "rollback.sh"}),
        json!({"phase": "backfill", "do": "backfill.sh: rewrites every item still on an old schema, conditional on it being unchanged since the scan", "local": "sspur deploy backfill --port N against deploy local"}),
        json!({"phase": "contract", "do": "after the old version is retired, a later version can drop migrate_/unmigrate_ functions whose schema no item has"}),
    ];
    json!({"from": old.hash, "to": new.hash, "service": new.name, "report": r.json(), "backfills": backfills, "phases": phases})
}
