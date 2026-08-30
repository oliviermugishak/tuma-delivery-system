//! Marketplace domain: merchants' businesses, their stores, and the
//! two-level catalog (marketplace schema).
//!
//! `stores` is the fulfillment boundary; `catalog` owns `products` (the
//! merchant-level identity of a thing they sell) and `store_products`
//! (per-store price/stock/availability) plus the product-image gallery
//! that dresses both. `geo` is the distance/ETA math the browse feed uses.
//!
//! Pure domain crate — no HTTP concerns (mirrors `accounts` and
//! `commerce`). Handlers in `tuma-server` call these functions with a
//! database connection and translate the typed errors.
//!
//! Invariants enforced here, never trusted to clients:
//! - Ownership resolves server-side from authorization scopes; a foreign
//!   resource is indistinguishable from a missing one.
//! - Money is integer RWF (`BIGINT`), enforced non-negative at the schema.
//! - Products are catalog identities; store_products are what customers
//!   actually buy.

pub mod catalog;
pub mod geo;
pub mod search;
pub mod stores;
