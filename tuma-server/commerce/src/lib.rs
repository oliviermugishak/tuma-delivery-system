//! Commerce domain: checkouts, order groups, store orders, deliveries,
//! payments — the transaction side of the platform. The catalog
//! (stores/products) lives in the `marketplace` crate; this crate owns
//! everything that happens after a customer taps "checkout". Tracking
//! rides on the deliveries table (build order #4).
//!
//! Pure domain crate — no HTTP concerns (mirrors `accounts`). Handlers
//! in `tuma-server` call these functions with a database connection and
//! translate the typed errors.
//!
//! Invariants enforced here, never trusted to clients:
//! - Money is integer RWF; every total is computed server-side.
//! - Item name + price are snapshotted at order time.
//! - Status moves along the six-value state machine, forward only.
//! - One checkout = one order group = one store order per participating
//!   store, created and committed atomically.
//! - One payment per checkout, explicitly allocated per store order.

pub mod orders;

pub use orders::{
    AllocationStatus, CancelError, CheckoutCreated, CheckoutError, GroupDetail, GroupStatus,
    MerchantStoreOrderRow, NewCheckout, NewCheckoutItem, OrderGroup, OrderItem, OrderStatus,
    Payment, PaymentAllocation, PaymentProvider, PaymentStatus, StockShort, StoreOrder,
    StoreOrderDetail, TransitionError,
};
