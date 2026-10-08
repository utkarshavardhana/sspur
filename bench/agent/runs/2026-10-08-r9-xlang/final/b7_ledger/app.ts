export interface Posting {
  readonly account: string;
  readonly amount: number;
}

export interface Txn {
  readonly id: number;
  readonly memo: string;
  readonly postings: readonly Posting[];
}

export interface Ledger {
  readonly accounts: readonly string[];
  readonly txns: readonly Txn[];
}

export class LedgerErr extends Error {}

export class UnknownAccount extends LedgerErr {
  constructor(readonly name: string) {
    super(name);
  }
}

export function chart(): string[] {
  return ["income:salary", "assets:bank", "expenses:food", "assets:cash", "equity:owner"];
}

export function newLedger(): Ledger {
  return { accounts: chart(), txns: [] };
}

export function p(account: string, amount: number): Posting {
  return { account, amount };
}

export function txn(id: number, memo: string, postings: readonly Posting[]): Txn {
  return { id, memo, postings };
}

export class Unbalanced extends LedgerErr {
  constructor(readonly id: number, readonly diff: number) {
    super(`unbalanced ${id}: ${diff}`);
  }
}

export class TooFewPostings extends LedgerErr {
  constructor(readonly id: number) {
    super(`too few postings ${id}`);
  }
}

export class DuplicateId extends LedgerErr {
  constructor(readonly id: number) {
    super(`duplicate id ${id}`);
  }
}

export class Overdrawn extends LedgerErr {
  constructor(readonly account: string, readonly balance: number) {
    super(`overdrawn ${account}: ${balance}`);
  }
}

export class UnknownTxn extends LedgerErr {
  constructor(readonly id: number) {
    super(`unknown txn ${id}`);
  }
}

export function addTxn(l: Ledger, t: Txn): Ledger {
  if (t.postings.length < 2) throw new TooFewPostings(t.id);
  for (const x of t.postings) {
    if (!l.accounts.includes(x.account)) throw new UnknownAccount(x.account);
  }
  const diff = t.postings.reduce((s, x) => s + x.amount, 0);
  if (diff !== 0) throw new Unbalanced(t.id, diff);
  if (l.txns.some((x) => x.id === t.id)) throw new DuplicateId(t.id);
  const next = { ...l, txns: [...l.txns, t] };
  for (const x of t.postings) {
    if (x.account.startsWith("assets:")) {
      const b = balance(next, x.account);
      if (b < 0) throw new Overdrawn(x.account, b);
    }
  }
  return next;
}

export function balance(l: Ledger, account: string): number {
  let s = 0;
  for (const t of l.txns) {
    for (const x of t.postings) {
      if (x.account === account) s += x.amount;
    }
  }
  return s;
}

export function trialBalance(l: Ledger): [string, number][] {
  const out: [string, number][] = [];
  for (const a of l.accounts) {
    const b = balance(l, a);
    if (b !== 0) out.push([a, b]);
  }
  return out.sort((x, y) => (x[0] < y[0] ? -1 : x[0] > y[0] ? 1 : 0));
}

export function reverse(l: Ledger, id: number, newId: number): Ledger {
  const t = l.txns.find((x) => x.id === id);
  if (!t) throw new UnknownTxn(id);
  return addTxn(l, txn(newId, `reverse ${id}`, t.postings.map((x) => p(x.account, 0 - x.amount))));
}

export function history(l: Ledger, account: string): [number, number][] {
  const out: [number, number][] = [];
  let s = 0;
  for (const t of l.txns) {
    let touched = false;
    for (const x of t.postings) {
      if (x.account === account) {
        s += x.amount;
        touched = true;
      }
    }
    if (touched) out.push([t.id, s]);
  }
  return out;
}

export function sample(): Ledger {
  const l = addTxn(newLedger(), txn(1, "open", [p("assets:bank", 1000), p("equity:owner", -1000)]));
  return addTxn(l, txn(2, "lunch", [p("expenses:food", 30), p("assets:bank", -30)]));
}
