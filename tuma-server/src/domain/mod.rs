//! Business logic that outlives any single HTTP route: typed errors and
//! functions over `&mut PgConnection`. Handlers stay thin and use `?` — the
//! `From<DomainError> for AppError` impls live in `crate::error`.

pub mod stores;
