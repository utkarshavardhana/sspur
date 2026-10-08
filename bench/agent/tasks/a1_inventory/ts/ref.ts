export interface Item {
  readonly sku: string;
  readonly name: string;
  readonly qty: number;
  readonly price: number;
}

export class InvErr extends Error {}

export class UnknownSku extends InvErr {
  constructor(readonly sku: string) {
    super(sku);
  }
}

export class OutOfStock extends InvErr {
  constructor(readonly sku: string, readonly wanted: number, readonly have: number) {
    super(`${sku}: wanted ${wanted}, have ${have}`);
  }
}

export function item(sku: string, name: string, qty: number, price: number): Item {
  return { sku, name, qty, price };
}

export function sample(): Item[] {
  return [item("c3", "cable", 7, 12), item("b2", "bolt", 40, 3), item("a1", "anchor", 2, 250)];
}

export function findItem(inv: readonly Item[], sku: string): Item | undefined {
  return inv.find((i) => i.sku === sku);
}

export function totalValue(inv: readonly Item[]): number {
  return inv.reduce((s, i) => s + i.qty * i.price, 0);
}

export function restock(inv: readonly Item[], sku: string, n: number): Item[] {
  if (n <= 0) throw new RangeError("n must be > 0");
  if (findItem(inv, sku) === undefined) throw new UnknownSku(sku);
  return inv.map((i) => (i.sku === sku ? { ...i, qty: i.qty + n } : i));
}

export function lowStock(inv: readonly Item[], threshold: number): string[] {
  return inv.filter((i) => i.qty < threshold).map((i) => i.sku).sort();
}

export class DuplicateSku extends InvErr {
  constructor(readonly sku: string) {
    super(sku);
  }
}

export function removeStock(inv: readonly Item[], sku: string, n: number): Item[] {
  if (n <= 0) throw new RangeError("n must be > 0");
  const it = findItem(inv, sku);
  if (it === undefined) throw new UnknownSku(sku);
  if (n > it.qty) throw new OutOfStock(sku, n, it.qty);
  return inv.map((i) => (i.sku === sku ? { ...i, qty: i.qty - n } : i));
}

export function addItem(inv: readonly Item[], it: Item): Item[] {
  if (findItem(inv, it.sku) !== undefined) throw new DuplicateSku(it.sku);
  return [...inv, it];
}

export function describe(i: Item): string {
  return `${i.sku} ${i.name} x${i.qty} @ ${i.price}`;
}

export function report(inv: readonly Item[]): string {
  return inv.map(describe).join("\n");
}
