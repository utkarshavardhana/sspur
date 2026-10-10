export interface Item { sku: string; qty: number; price: number }
export interface Order { id: string; items: Item[] }

export type OrderError =
  | { kind: "empty" }
  | { kind: "outOfStock"; sku: string }
  | { kind: "badQty"; sku: string };

export interface Store {
  stock(sku: string): Promise<number>;
  putOrder(order: Order): Promise<void>;
}

export type Result<T, E> = { ok: true; value: T } | { ok: false; error: E };

export function total(items: Item[]): number {
  return items.reduce((s, i) => s + i.price * i.qty, 0);
}

export async function place(store: Store, order: Order): Promise<Result<Order, OrderError>> {
  if (order.items.length === 0) return { ok: false, error: { kind: "empty" } };
  for (const i of order.items) {
    if (i.qty <= 0) return { ok: false, error: { kind: "badQty", sku: i.sku } };
    if ((await store.stock(i.sku)) < i.qty) {
      return { ok: false, error: { kind: "outOfStock", sku: i.sku } };
    }
  }
  await store.putOrder(order);
  return { ok: true, value: order };
}
