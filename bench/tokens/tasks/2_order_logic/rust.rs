use rust_decimal::Decimal;

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub sku: String,
    pub qty: u32,
    pub price: Decimal,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub id: String,
    pub items: Vec<Item>,
}

#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    #[error("empty order")]
    Empty,
    #[error("bad qty for {0}")]
    BadQty(String),
    #[error("out of stock: {0}")]
    OutOfStock(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Debug, thiserror::Error)]
#[error("store: {0}")]
pub struct StoreError(pub String);

pub trait Store {
    fn stock(&self, sku: &str) -> Result<u32, StoreError>;
    fn put_order(&self, order: &Order) -> Result<(), StoreError>;
}

pub fn total(items: &[Item]) -> Decimal {
    items.iter().map(|i| i.price * Decimal::from(i.qty)).sum()
}

pub fn place(store: &impl Store, order: Order) -> Result<Order, OrderError> {
    if order.items.is_empty() {
        return Err(OrderError::Empty);
    }
    for i in &order.items {
        if i.qty == 0 {
            return Err(OrderError::BadQty(i.sku.clone()));
        }
        if store.stock(&i.sku)? < i.qty {
            return Err(OrderError::OutOfStock(i.sku.clone()));
        }
    }
    store.put_order(&order)?;
    Ok(order)
}
