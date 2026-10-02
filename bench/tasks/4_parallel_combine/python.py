import asyncio
from dataclasses import dataclass
from decimal import Decimal


@dataclass(frozen=True)
class Summary:
    name: str
    item_count: int
    total: Decimal


async def summary(api, user_id: str) -> Summary:
    async with asyncio.TaskGroup() as tg:
        u = tg.create_task(api.get_user(user_id))
        c = tg.create_task(api.get_cart(user_id))
    user, cart = u.result(), c.result()
    return Summary(
        name=user.name,
        item_count=len(cart.items),
        total=sum((i.price * i.qty for i in cart.items), Decimal(0)),
    )
