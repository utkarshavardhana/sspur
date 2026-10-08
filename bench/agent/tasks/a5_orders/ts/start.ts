export interface Line {
  readonly sku: string;
  readonly qty: number;
  readonly unit: number;
}

export interface Order {
  readonly id: number;
  readonly lines: readonly Line[];
  readonly coupon: string;
}

export function line(sku: string, qty: number, unit: number): Line {
  return { sku, qty, unit };
}

export function order(id: number, lines: readonly Line[], coupon: string): Order {
  return { id, lines, coupon };
}

export function subtotal(o: Order): number {
  return o.lines.reduce((s, l) => s + l.qty * l.unit, 0);
}

export function shipping(o: Order): number {
  return subtotal(o) >= 5000 ? 0 : 499;
}

export function tax(o: Order): number {
  return Math.floor((subtotal(o) * 8) / 100);
}

export function total(o: Order): number {
  return subtotal(o) + shipping(o) + tax(o);
}

export function money(cents: number): string {
  if (cents < 0) throw new RangeError("cents must be >= 0");
  return `${Math.floor(cents / 100)}.${String(cents % 100).padStart(2, "0")}`;
}

export function invoice(o: Order): string {
  return `order ${o.id}: subtotal ${money(subtotal(o))}, shipping ${money(shipping(o))}, tax ${money(tax(o))}, total ${money(total(o))}`;
}

export function sampleOrder(): Order {
  return order(7, [line("pen", 3, 250), line("pad", 2, 1200)], "");
}
