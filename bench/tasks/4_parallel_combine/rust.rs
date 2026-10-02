use rust_decimal::Decimal;

pub struct Summary {
    pub name: String,
    pub item_count: usize,
    pub total: Decimal,
}

pub async fn summary(api: &impl Api, user_id: &str) -> Result<Summary, ApiError> {
    let (user, cart) = tokio::try_join!(api.get_user(user_id), api.get_cart(user_id))?;
    Ok(Summary {
        name: user.name,
        item_count: cart.items.len(),
        total: cart.items.iter().map(|i| i.price * Decimal::from(i.qty)).sum(),
    })
}
