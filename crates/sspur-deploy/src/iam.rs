use crate::{pascal, Handler, Service};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub fn action(op: &str) -> &'static str {
    match op {
        "get" => "dynamodb:GetItem",
        "put" => "dynamodb:PutItem",
        "del" => "dynamodb:DeleteItem",
        _ => "dynamodb:Scan",
    }
}

pub const LOG_ACTIONS: [&str; 2] = ["logs:CreateLogStream", "logs:PutLogEvents"];

pub fn table_id(store: &str) -> String {
    format!("{}Table", pascal(store))
}

pub fn fn_id(handler: &str) -> String {
    pascal(handler)
}

pub fn actions(h: &Handler) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (store, op) in &h.db {
        let v = out.entry(store.clone()).or_default();
        let a = action(op).to_string();
        if !v.contains(&a) {
            v.push(a);
        }
    }
    out.values_mut().for_each(|v| v.sort());
    out
}

pub fn statements(h: &Handler) -> Vec<Value> {
    let mut out: Vec<Value> = actions(h)
        .into_iter()
        .map(|(store, acts)| json!({"Sid": format!("Db{}", pascal(&store)), "Effect": "Allow", "Action": acts, "Resource": {"Fn::GetAtt": [table_id(&store), "Arn"]}}))
        .collect();
    out.push(json!({"Sid": "Logs", "Effect": "Allow", "Action": LOG_ACTIONS, "Resource": {"Fn::GetAtt": [format!("{}Logs", fn_id(&h.name)), "Arn"]}}));
    out
}

pub fn policy(_svc: &Service, h: &Handler) -> Value {
    json!({"Version": "2012-10-17", "Statement": statements(h)})
}

pub fn summary(svc: &Service) -> Value {
    let handlers: Vec<Value> = svc
        .handlers
        .iter()
        .map(|h| {
            let routes: Vec<String> = svc.routes.iter().filter(|r| r.handler == h.name).map(|r| r.key()).collect();
            let ops: Vec<String> = h.db.iter().map(|(s, o)| format!("db.{o}({s})")).collect();
            json!({"handler": h.name, "routes": routes, "effects": h.row, "db_ops": ops, "actions": actions(h), "validator": h.validator})
        })
        .collect();
    let stores: Vec<Value> = svc
        .stores
        .iter()
        .map(|s| {
            let migs: Vec<Value> = s.migs.iter().map(|g| json!({"from": g.sv, "fn": g.fun, "reverse": s.backs.iter().find(|b| b.sv == g.sv).map(|b| &b.fun)})).collect();
            json!({"store": s.name, "key": s.key.to_string(), "value": s.val.to_string(), "schema": s.sv, "migrations": migs, "resource": table_id(&s.name)})
        })
        .collect();
    let backfills: Vec<Value> = svc.backfills.iter().map(|b| json!({"function": b.name, "actions": actions(b)})).collect();
    json!({"service": svc.name, "hash": svc.hash, "stores": stores, "handlers": handlers, "backfills": backfills})
}
