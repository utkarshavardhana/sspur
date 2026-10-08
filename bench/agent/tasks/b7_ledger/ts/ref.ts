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

export class Unbalanced extends LedgerErr {
  constructor(readonly id: number, readonly diff: number) {
    super(`txn ${id} is off by ${diff}`);
  }
}

export class TooFewPostings extends LedgerErr {
  constructor(readonly id: number) {
    super(`txn ${id} has fewer than 2 postings`);
  }
}

export class DuplicateId extends LedgerErr {
  constructor(readonly id: number) {
    super(`duplicate txn ${id}`);
  }
}

export class Overdrawn extends LedgerErr {
  constructor(readonly account: string, readonly balance: number) {
    super(`${account} would be ${balance}`);
  }
}

export class UnknownTxn extends LedgerErr {
  constructor(readonly id: number) {
    super(`no txn ${id}`);
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

export function addTxn(l: Ledger, t: Txn): Ledger {
  if (t.postings.length < 2) throw new TooFewPostings(t.id);
  for (const x of t.postings) {
    if (!l.accounts.includes(x.account)) throw new UnknownAccount(x.account);
  }
  const diff = t.postings.reduce((s, x) => s + x.amount, 0);
  if (diff !== 0) throw new Unbalanced(t.id, diff);
  if (l.txns.some((u) => u.id === t.id)) throw new DuplicateId(t.id);
  const l2 = { ...l, txns: [...l.txns, t] };
  for (const x of t.postings) {
    if (x.account.startsWith("assets:") && balance(l2, x.account) < 0) {
      throw new Overdrawn(x.account, balance(l2, x.account));
    }
  }
  return l2;
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
  return [...l.accounts]
    .sort()
    .filter((a) => balance(l, a) !== 0)
    .map((a) => [a, balance(l, a)]);
}

export function reverse(l: Ledger, id: number, newId: number): Ledger {
  for (const t of l.txns) {
    if (t.id === id) {
      return addTxn(l, txn(newId, `reverse ${id}`, t.postings.map((x) => ({ ...x, amount: -x.amount }))));
    }
  }
  throw new UnknownTxn(id);
}

export function history(l: Ledger, account: string): [number, number][] {
  let bal = 0;
  const out: [number, number][] = [];
  for (const t of l.txns) {
    const ps = t.postings.filter((x) => x.account === account);
    if (ps.length > 0) {
      bal += ps.reduce((s, x) => s + x.amount, 0);
      out.push([t.id, bal]);
    }
  }
  return out;
}

export function sample(): Ledger {
  const l = addTxn(newLedger(), txn(1, "open", [p("assets:bank", 1000), p("equity:owner", -1000)]));
  return addTxn(l, txn(2, "lunch", [p("expenses:food", 30), p("assets:bank", -30)]));
}
