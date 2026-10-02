export interface Summary { name: string; itemCount: number; total: number }

export async function summary(api: Api, userId: string): Promise<Summary> {
  const [user, cart] = await Promise.all([api.getUser(userId), api.getCart(userId)]);
  return {
    name: user.name,
    itemCount: cart.items.length,
    total: cart.items.reduce((s, i) => s + i.price * i.qty, 0),
  };
}
