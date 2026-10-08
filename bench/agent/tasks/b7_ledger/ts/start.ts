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

export function addTxn(l: Ledger, t: Txn): Ledger {
  for (const x of t.postings) {
    if (!l.accounts.includes(x.account)) throw new UnknownAccount(x.account);
  }
  return { ...l, txns: [...l.txns, t] };
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
  return l.accounts.map((a) => [a, balance(l, a)]);
}

export function sample(): Ledger {
  const l = addTxn(newLedger(), txn(1, "open", [p("assets:bank", 1000), p("equity:owner", -1000)]));
  return addTxn(l, txn(2, "lunch", [p("expenses:food", 30), p("assets:bank", -30)]));
}
