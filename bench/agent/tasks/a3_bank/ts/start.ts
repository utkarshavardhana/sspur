export interface Account {
  readonly id: string;
  readonly owner: string;
  readonly balance: number;
}

export class BankErr extends Error {}

export class Insufficient extends BankErr {
  constructor(readonly id: string, readonly needed: number, readonly available: number) {
    super(`${id}: needed ${needed}, available ${available}`);
  }
}

export class SameAccount extends BankErr {}

export class UnknownAccount extends BankErr {
  constructor(readonly id: string) {
    super(id);
  }
}

export function account(id: string, owner: string, balance: number): Account {
  return { id, owner, balance };
}

export function accounts(): Account[] {
  return [account("a1", "alice", 100), account("b1", "bob", 20), account("c1", "carol", 0)];
}

export function openAccount(bank: readonly Account[], id: string, owner: string): Account[] {
  return [...bank, account(id, owner, 0)];
}

export function lookup(bank: readonly Account[], id: string): Account {
  const a = bank.find((a) => a.id === id);
  if (a === undefined) throw new UnknownAccount(id);
  return a;
}

export function setBalance(bank: readonly Account[], id: string, b: number): Account[] {
  if (b < 0) throw new RangeError("balance must be >= 0");
  return bank.map((a) => (a.id === id ? { ...a, balance: b } : a));
}

export function deposit(bank: readonly Account[], id: string, amount: number): Account[] {
  if (amount <= 0) throw new RangeError("amount must be > 0");
  const a = lookup(bank, id);
  return setBalance(bank, id, a.balance + amount);
}

export function withdraw(bank: readonly Account[], id: string, amount: number): Account[] {
  if (amount <= 0) throw new RangeError("amount must be > 0");
  const a = lookup(bank, id);
  if (amount > a.balance) throw new Insufficient(id, amount, a.balance);
  return setBalance(bank, id, a.balance - amount);
}

export function transfer(bank: readonly Account[], frm: string, to: string, amount: number): Account[] {
  if (frm === to) throw new SameAccount();
  return deposit(withdraw(bank, frm, amount), to, amount);
}

export function describe(a: Account): string {
  return `${a.owner}: ${a.balance}`;
}

export function attempt(bank: readonly Account[], frm: string, to: string, amount: number): string {
  try {
    return transfer(bank, frm, to, amount).map(describe).join(", ");
  } catch (e) {
    if (e instanceof Insufficient) return `declined: ${e.id} needs ${e.needed}, has ${e.available}`;
    if (e instanceof SameAccount) return "declined: same account";
    if (e instanceof UnknownAccount) return `declined: no account ${e.id}`;
    throw e;
  }
}
