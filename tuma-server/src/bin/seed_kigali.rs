//! Seeds the Kigali pilot: three real merchant businesses, seven real
//! branches (with real, map-verifiable coordinates), and their catalogs
//! and per-store assortments.
//!
//! Usage:
//!   cargo run --bin seed_kigali
//!   ./run.sh seed
//!
//! Runs pending migrations first (like `seed_admin`). Idempotent: if any
//! merchant business already exists, it leaves the database alone.
//! Owner sign-ins are documented in MEMORY.md — the founder should change
//! them before anything real.

use marketplace::catalog::{self, NewStoreProduct};
use marketplace::stores::{self, StoreChanges};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use tuma_server::config::get_configuration;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const OWNER_PASSWORD: &str = "Password123";

struct StoreSeed {
    name: &'static str,
    address: &'static str,
    lat: f64,
    lng: f64,
    category: &'static str,
    delivery_fee: i64,
    /// The customer-facing contact surface (the app's Get-help sheet).
    contact_phone: &'static str,
    contact_email: &'static str,
    /// (catalog product, price at this branch, stock: None = untracked)
    menu: &'static [(&'static str, i64, Option<i64>)],
}

struct MerchantSeed {
    business: &'static str,
    business_email: &'static str,
    owner_email: &'static str,
    stores: &'static [StoreSeed],
}

const SIMBA_MENU: &[(&str, i64, Option<i64>)] = &[
    ("Rice 5KG", 12000, Some(40)),
    ("Inyange Milk 1L", 1500, Some(80)),
    ("Sugar 1KG", 1800, Some(60)),
    ("Cooking Oil 5L", 14000, Some(20)),
    ("White Bread 400g", 1200, None),
    ("Eggs (Tray of 30)", 4500, Some(15)),
    ("Azam Water 1.5L", 1000, Some(200)),
    ("Laundry Soap Bar", 800, None),
];

const SEEDS: &[MerchantSeed] = &[
    MerchantSeed {
        business: "Simba Supermarket",
        business_email: "info@simbasupermarket.rw",
        owner_email: "simba@tuma.rw",
        stores: &[
            StoreSeed {
                name: "Simba Supermarket Remera",
                address: "KG 7 Ave, Remera, Kigali",
                lat: -1.9620,
                lng: 30.1290,
                category: "Supermarket",
                delivery_fee: 1500,
                contact_phone: "+250788100001",
                contact_email: "remera@simbasupermarket.rw",
                // Branch pricing: Kimironko slightly higher, downtown promo.
                menu: SIMBA_MENU,
            },
            StoreSeed {
                name: "Simba Supermarket Kimironko",
                address: "Kimironko Market Road, Kigali",
                lat: -1.9390,
                lng: 30.1255,
                category: "Supermarket",
                delivery_fee: 1500,
                contact_phone: "+250788100002",
                contact_email: "kimironko@simbasupermarket.rw",
                menu: &[
                    ("Rice 5KG", 12500, Some(25)),
                    ("Inyange Milk 1L", 1600, Some(50)),
                    ("Sugar 1KG", 1900, Some(30)),
                    ("Cooking Oil 5L", 14500, Some(12)),
                    ("White Bread 400g", 1300, None),
                    ("Eggs (Tray of 30)", 4800, Some(10)),
                    ("Azam Water 1.5L", 1000, Some(150)),
                    ("Laundry Soap Bar", 900, None),
                ],
            },
            StoreSeed {
                name: "Simba Supermarket City Center",
                address: "KN 4 Ave, Nyarugenge (UTC), Kigali",
                lat: -1.9499,
                lng: 30.0622,
                category: "Supermarket",
                delivery_fee: 1000,
                contact_phone: "+250788100003",
                contact_email: "citycenter@simbasupermarket.rw",
                menu: &[
                    ("Rice 5KG", 11500, Some(60)),
                    ("Inyange Milk 1L", 1500, Some(100)),
                    ("Sugar 1KG", 1800, Some(70)),
                    ("Cooking Oil 5L", 13900, Some(25)),
                    ("White Bread 400g", 1200, None),
                    ("Eggs (Tray of 30)", 4500, Some(20)),
                    ("Azam Water 1.5L", 900, Some(250)),
                    ("Laundry Soap Bar", 800, None),
                ],
            },
        ],
    },
    MerchantSeed {
        business: "KFC Rwanda",
        business_email: "info@kfcrwanda.rw",
        owner_email: "kfc@tuma.rw",
        stores: &[
            StoreSeed {
                name: "KFC Kigali Heights",
                address: "KG 7 Ave, Kimihurura, Kigali",
                lat: -1.9410,
                lng: 30.0912,
                category: "Fast food",
                delivery_fee: 2000,
                contact_phone: "+250788200001",
                contact_email: "heights@kfcrwanda.rw",
                menu: &[
                    ("Streetwise 2 (Chicken & Fries)", 6500, None),
                    ("Zinger Burger", 7000, None),
                    ("Regular Fries", 3000, None),
                    ("5pc Hot Wings", 5500, None),
                    ("Soda 500ml", 1000, None),
                ],
            },
            StoreSeed {
                name: "KFC Remera",
                address: "KG 7 Ave, Remera (Airport Road), Kigali",
                lat: -1.9635,
                lng: 30.1320,
                category: "Fast food",
                delivery_fee: 1500,
                contact_phone: "+250788200002",
                contact_email: "remera@kfcrwanda.rw",
                menu: &[
                    ("Streetwise 2 (Chicken & Fries)", 6500, None),
                    ("Zinger Burger", 7000, None),
                    ("Regular Fries", 3000, None),
                    ("5pc Hot Wings", 5500, None),
                    ("Soda 500ml", 1000, None),
                ],
            },
        ],
    },
    MerchantSeed {
        business: "Java House Rwanda",
        business_email: "info@javahouse.rw",
        owner_email: "java@tuma.rw",
        stores: &[
            StoreSeed {
                name: "Java House Kiyovu",
                address: "KN 2 Ave, Kiyovu, Kigali",
                lat: -1.9580,
                lng: 30.0575,
                category: "Coffee shop",
                delivery_fee: 1000,
                contact_phone: "+250788300001",
                contact_email: "kiyovu@javahtourer.rw",
                menu: &[
                    ("Cappuccino", 3500, None),
                    ("Espresso", 2500, None),
                    ("Big Breakfast Platter", 8500, None),
                    ("Grilled Chicken Sandwich", 6000, None),
                    ("Passion Fruit Fresh Juice", 3000, None),
                ],
            },
            StoreSeed {
                name: "Java House Nyarutarama",
                address: "KG 9 Ave, Nyarutarama, Kigali",
                lat: -1.9352,
                lng: 30.0910,
                category: "Coffee shop",
                delivery_fee: 1000,
                contact_phone: "+250788300002",
                contact_email: "nyarutarama@javahtourer.rw",
                menu: &[
                    ("Cappuccino", 3500, None),
                    ("Espresso", 2500, None),
                    ("Big Breakfast Platter", 8500, None),
                    ("Grilled Chicken Sandwich", 6000, None),
                    ("Passion Fruit Fresh Juice", 3000, None),
                ],
            },
        ],
    },
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = get_configuration().expect("Failed to get configuration");
    let pool = PgPoolOptions::new()
        .connect_with(config.database_with_db())
        .await?;
    MIGRATOR.run(&pool).await?;

    let mut conn = pool.acquire().await?;

    // Idempotency guard, simple and honest: a non-empty marketplace is not
    // ours to touch.
    if accounts::merchants::count(&mut conn).await? > 0 {
        println!("marketplace already seeded — leaving it alone");
        return Ok(());
    }

    let accounts_manager = accounts::AccountManager::new(2);
    let mut store_count = 0usize;
    let mut item_count = 0usize;

    for merchant_seed in SEEDS {
        let merchant = accounts::merchants::create(
            &mut conn,
            merchant_seed.business,
            Some(merchant_seed.business_email),
            None,
        )
        .await?;
        let owner = accounts_manager
            .create_password_account(&mut conn, merchant_seed.owner_email, OWNER_PASSWORD)
            .await?;
        accounts::memberships::create(
            &mut conn,
            owner.id,
            merchant.id,
            accounts::MembershipRole::Owner,
            None,
        )
        .await?;
        println!(
            "business: {} (owner {} / {OWNER_PASSWORD})",
            merchant_seed.business, merchant_seed.owner_email
        );

        for store_seed in merchant_seed.stores {
            let store = stores::create_store(
                &mut conn,
                merchant.id,
                StoreChanges {
                    name: store_seed.name.to_string(),
                    description: Some(format!(
                        "{} — {} branch, Kigali.",
                        merchant_seed.business,
                        store_seed
                            .address
                            .split(',')
                            .next()
                            .unwrap_or(store_seed.name)
                    )),
                    image_url: None,
                    address_text: Some(store_seed.address.to_string()),
                    lat: Some(store_seed.lat),
                    lng: Some(store_seed.lng),
                    category: Some(store_seed.category.to_string()),
                    delivery_fee: store_seed.delivery_fee,
                    contact_phone: Some(store_seed.contact_phone.to_string()),
                    contact_email: Some(store_seed.contact_email.to_string()),
                    // Open for business immediately — the founder tests
                    // against these today.
                    is_open: true,
                },
            )
            .await?;

            // The catalog product is created once per merchant (keyed by
            // name); each store gets its own price + stock.
            let mut product_ids = std::collections::HashMap::new();
            for (product_name, price, stock) in store_seed.menu {
                let product_id = match product_ids.get(*product_name) {
                    Some(id) => *id,
                    None => {
                        let product = catalog::create_product(
                            &mut conn,
                            merchant.id,
                            (*product_name).to_string(),
                            None,
                            None,
                        )
                        .await?;
                        product_ids.insert(*product_name, product.id);
                        product.id
                    }
                };
                catalog::create_store_product(
                    &mut conn,
                    merchant.id,
                    NewStoreProduct {
                        store_id: store.id,
                        product_id,
                        price: *price,
                        stock: *stock,
                        is_available: true,
                        sku: None,
                    },
                )
                .await?;
                item_count += 1;
            }
            store_count += 1;
            println!(
                "  store: {} ({}, {:.4}, {:.4}) — {} items",
                store_seed.name,
                store_seed.address,
                store_seed.lat,
                store_seed.lng,
                store_seed.menu.len()
            );
        }
    }

    println!(
        "seeded {store_count} stores, {item_count} store products across {} businesses",
        SEEDS.len()
    );
    Ok(())
}
