//! Image storage end to end (slice U1): banner and gallery uploads flow
//! through validation → normalization → object store → DB keys, and the
//! public file route serves what was stored. The in-memory backend keeps
//! every assertion hermetic.

mod common;

use common::{
    MIGRATOR, TestClient, seed_merchant, seed_store, seed_store_manager, spawn_app, token_for,
};
use reqwest::multipart::{Form, Part};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

struct Operator {
    client: TestClient,
    token: String,
    merchant_id: Uuid,
}

async fn owner(app: &common::TestApp, email: &str) -> Operator {
    let seeded = seed_merchant(&app.pool, email, "Aline's Kitchen").await;
    Operator {
        client: TestClient::new(&app.address),
        token: token_for(app, seeded.account.id, 3600),
        merchant_id: seeded.merchant.id,
    }
}

/// A valid 64×32 PNG — small enough to pass through normalization
/// unchanged in size.
fn png_bytes() -> Vec<u8> {
    let image = image::RgbaImage::from_pixel(64, 32, image::Rgba([255, 0, 0, 255]));
    let mut buffer = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut buffer),
            image::ImageFormat::Png,
        )
        .expect("test PNG encodes");
    buffer
}

/// A minimal PNG whose IHDR declares 20000×20000 while the whole file is
/// a few hundred bytes — the decode bomb's shape (review S33). Built byte
/// by byte: actually encoding a real 20000px image would allocate the
/// very gigabytes the gate exists to prevent. CRC-32 (IEEE) is computed
/// honestly because the PNG decoder validates chunk checksums.
fn png_dimension_bomb_bytes(width: u32, height: u32) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xFFFF_FFFF;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }
    fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
        let mut chunk = Vec::with_capacity(12 + data.len());
        chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
        chunk.extend_from_slice(kind);
        chunk.extend_from_slice(data);
        let mut crc_input = Vec::with_capacity(4 + data.len());
        crc_input.extend_from_slice(kind);
        crc_input.extend_from_slice(data);
        chunk.extend_from_slice(&crc32(&crc_input).to_be_bytes());
        chunk
    }

    let ihdr = &[
        &width.to_be_bytes()[..],
        &height.to_be_bytes()[..],
        &[8, 2, 0, 0, 0], // 8-bit depth, truecolor RGB, no interlace
    ]
    .concat();
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    png.extend_from_slice(&chunk(b"IHDR", ihdr));
    // A stub zlib stream: the gate must refuse on the header BEFORE any
    // decompression is attempted, so the payload never has to be valid.
    png.extend_from_slice(&chunk(b"IDAT", &[0x78, 0x01, 0x01, 0x00, 0x00, 0xFF, 0xFF]));
    png.extend_from_slice(&chunk(b"IEND", &[]));
    png
}

fn png_form() -> Form {
    Form::new().part(
        "file",
        Part::bytes(png_bytes())
            .file_name("kitchen.jpg")
            .mime_str("image/png")
            .expect("mime"),
    )
}

/// Upload through the multipart endpoint; assertions read the response.
async fn upload_banner(operator: &Operator, store_id: Uuid) -> reqwest::Response {
    operator
        .client
        .post(&format!("/v1/merchant/stores/{store_id}/banner"))
        .bearer_auth(&operator.token)
        .multipart(png_form())
        .send()
        .await
        .expect("request completes")
}

async fn create_store_via_api(operator: &Operator) -> Uuid {
    let response = operator
        .client
        .post_json(
            "/v1/merchant/stores",
            json!({ "name": "Aline's Kitchen", "delivery_fee": 1000 }),
        )
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: Value = response.json().await.unwrap();
    body["id"].as_str().unwrap().parse().unwrap()
}

async fn create_product_via_api(operator: &Operator) -> Uuid {
    let response = operator
        .client
        .post_json("/v1/merchant/products", json!({ "name": "Ibirazi" }))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
    let body: Value = response.json().await.unwrap();
    body["id"].as_str().unwrap().parse().unwrap()
}

/// The storage key lives between the …/files/ marker and the end of the
/// URL; the tests need it to fetch the object back through the file route.
fn key_from_url(image_url: &str) -> &str {
    image_url
        .split("/api/v1/files/")
        .nth(1)
        .expect("the URL carries the file route marker")
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn banner_upload_flows_end_to_end(pool: PgPool) {
    let app = spawn_app(pool).await;
    let operator = owner(&app, "owner@tuma.rw").await;
    let store_id = create_store_via_api(&operator).await;

    let response = upload_banner(&operator, store_id).await;
    assert_eq!(response.status(), 201, "body: {:?}", response.text().await);
    let body: Value = response.json().await.unwrap();

    // The composed URL points at the file route with a generated .jpg key.
    let image_url = body["image_url"].as_str().unwrap();
    let key = key_from_url(image_url);
    assert!(key.starts_with(&format!(
        "merchants/{}/stores/{store_id}/banner-",
        operator.merchant_id
    )));
    assert!(key.ends_with(".jpg"));

    // The stored bytes come back through the public route, immutable and
    // correctly typed (the sniffed PNG went in, a JPEG comes out).
    let served = operator
        .client
        .get(&format!("/v1/files/{key}"))
        .send()
        .await
        .unwrap();
    assert_eq!(served.status(), 200);
    assert_eq!(served.headers()["content-type"], "image/jpeg");
    assert_eq!(
        served.headers()["cache-control"],
        "public, max-age=31536000, immutable"
    );
    let bytes = served.bytes().await.unwrap();
    assert_eq!(&bytes[..2], b"\xff\xd8", "served bytes are JPEG");

    // The row now carries the key.
    let mut conn = app.pool.acquire().await.unwrap();
    let stored: Option<String> =
        sqlx::query_scalar("SELECT banner_key FROM marketplace.stores WHERE id = $1")
            .bind(store_id)
            .fetch_one(&mut *conn)
            .await
            .unwrap();
    assert_eq!(stored.as_deref(), Some(key));
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn banner_replace_retires_the_old_object(pool: PgPool) {
    let app = spawn_app(pool).await;
    let operator = owner(&app, "owner@tuma.rw").await;
    let store_id = create_store_via_api(&operator).await;

    let first: Value = upload_banner(&operator, store_id)
        .await
        .json()
        .await
        .unwrap();
    let first_key = key_from_url(first["image_url"].as_str().unwrap()).to_string();

    let second: Value = upload_banner(&operator, store_id)
        .await
        .json()
        .await
        .unwrap();
    let second_key = key_from_url(second["image_url"].as_str().unwrap()).to_string();
    assert_ne!(first_key, second_key, "a replace writes a NEW key");

    let old = operator
        .client
        .get(&format!("/v1/files/{first_key}"))
        .send()
        .await
        .unwrap();
    assert_eq!(old.status(), 404, "the replaced banner is gone");
    let new = operator
        .client
        .get(&format!("/v1/files/{second_key}"))
        .send()
        .await
        .unwrap();
    assert_eq!(new.status(), 200);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn banner_belongs_to_reachable_stores_only(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@tuma.rw").await;
    let bruce = owner(&app, "bruce@tuma.rw").await;
    let foreign_store = seed_store(&app.pool, bruce.merchant_id, "Bruce's Grill", false).await;

    // Another merchant's store is indistinguishable from a missing one…
    let response = upload_banner(&aline, foreign_store.id).await;
    assert_eq!(response.status(), 404);

    // …and an anonymous request never gets in at all.
    let anonymous = TestClient::new(&app.address)
        .post(&format!("/v1/merchant/stores/{}/banner", foreign_store.id))
        .multipart(png_form())
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn a_store_scoped_manager_can_dress_their_store(pool: PgPool) {
    let app = spawn_app(pool).await;
    let owner_session = owner(&app, "owner@tuma.rw").await;
    let store_id = create_store_via_api(&owner_session).await;

    let manager_account = seed_store_manager(
        &app.pool,
        "manager@tuma.rw",
        owner_session.merchant_id,
        store_id,
    )
    .await;
    let manager = Operator {
        client: TestClient::new(&app.address),
        token: token_for(&app, manager_account.id, 3600),
        merchant_id: owner_session.merchant_id,
    };
    let response = upload_banner(&manager, store_id).await;
    assert_eq!(response.status(), 201);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn uploads_reject_non_images_and_oversize(pool: PgPool) {
    let app = spawn_app(pool).await;
    let operator = owner(&app, "owner@tuma.rw").await;
    let store_id = create_store_via_api(&operator).await;

    // Text wearing an .jpg name: the sniff is by content, not by name.
    let text_form = Form::new().part(
        "file",
        Part::bytes("this is not an image".as_bytes())
            .file_name("fake.jpg")
            .mime_str("image/jpeg")
            .unwrap(),
    );
    let response = operator
        .client
        .post(&format!("/v1/merchant/stores/{store_id}/banner"))
        .bearer_auth(&operator.token)
        .multipart(text_form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 415);

    // A JPEG-shaped lie (magic bytes, garbage body) is still a 415, not
    // a 500 — corrupt bytes are the client's fault.
    let fake_form = Form::new().part(
        "file",
        Part::bytes(&b"\xff\xd8\xff\xe0garbage-garbage-garbage"[..])
            .file_name("truncated.jpg")
            .mime_str("image/jpeg")
            .unwrap(),
    );
    let response = operator
        .client
        .post(&format!("/v1/merchant/stores/{store_id}/banner"))
        .bearer_auth(&operator.token)
        .multipart(fake_form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 415);

    // The size cap answers 413 before any decoding — the bytes need not
    // be a valid image to be told they're too big.
    let big_form = Form::new().part(
        "file",
        Part::bytes(vec![0u8; 5 * 1024 * 1024 + 1])
            .file_name("big.jpg")
            .mime_str("image/jpeg")
            .unwrap(),
    );
    let response = operator
        .client
        .post(&format!("/v1/merchant/stores/{store_id}/banner"))
        .bearer_auth(&operator.token)
        .multipart(big_form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413);
}

/// Review S33: the dimension gate reads the container header and refuses
/// a decode bomb with the byte cap's status — 413 — BEFORE decoding, so a
/// few hundred hostile bytes cannot allocate gigabytes (a 20000×20000
/// buffer) or hang the worker. Not a 415, not a 500, not an OOM.
#[sqlx::test(migrator = "MIGRATOR")]
async fn an_oversized_dimension_upload_is_rejected_before_decoding(pool: PgPool) {
    let app = spawn_app(pool).await;
    let operator = owner(&app, "owner@tuma.rw").await;
    let store_id = create_store_via_api(&operator).await;

    let bomb = png_dimension_bomb_bytes(20_000, 20_000);
    assert!(
        bomb.len() < 500,
        "the bomb must be tiny on the wire: {} bytes",
        bomb.len()
    );
    let bomb_form = Form::new().part(
        "file",
        Part::bytes(bomb)
            .file_name("bomb.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let response = operator
        .client
        .post(&format!("/v1/merchant/stores/{store_id}/banner"))
        .bearer_auth(&operator.token)
        .multipart(bomb_form)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        413,
        "declared-size excess is the byte cap's class, not a decode error"
    );

    // The ceiling is real but generous: 12000×9000 — a 108 MP phone
    // sensor's full output — passes the gate. The stub payload then fails
    // the DECODE (415): 415 here is the proof the dimension gate let it
    // through, without encoding a real 108 MP image in the test.
    let legal = png_dimension_bomb_bytes(12_000, 9_000);
    let legal_form = Form::new().part(
        "file",
        Part::bytes(legal)
            .file_name("big-phone.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let response = operator
        .client
        .post(&format!("/v1/merchant/stores/{store_id}/banner"))
        .bearer_auth(&operator.token)
        .multipart(legal_form)
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        415,
        "at the ceiling the gate passes; only the stub decode fails"
    );

    // One pixel over the ceiling on either axis is refused again.
    let over = png_dimension_bomb_bytes(12_001, 9_000);
    let over_form = Form::new().part(
        "file",
        Part::bytes(over)
            .file_name("over.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let response = operator
        .client
        .post(&format!("/v1/merchant/stores/{store_id}/banner"))
        .bearer_auth(&operator.token)
        .multipart(over_form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 413, "12_001 wide is over the line");
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn gallery_positions_cover_and_customer_view(pool: PgPool) {
    let app = spawn_app(pool).await;
    let operator = owner(&app, "owner@tuma.rw").await;
    let product_id = create_product_via_api(&operator).await;

    let mut urls: Vec<String> = Vec::new();
    for _ in 0..3 {
        let response = operator
            .client
            .post(&format!("/v1/merchant/products/{product_id}/images"))
            .bearer_auth(&operator.token)
            .multipart(png_form())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
        let body: Value = response.json().await.unwrap();
        urls.push(body["image_url"].as_str().unwrap().to_string());
    }

    // Appended in order: 0, 1, 2 — cover first.
    let list: Value = operator
        .client
        .get(&format!("/v1/merchant/products/{product_id}/images"))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let images = list.as_array().unwrap();
    assert_eq!(images.len(), 3);
    assert_eq!(images[0]["image_url"], Value::String(urls[0].clone()));
    let positions: Vec<i64> = images
        .iter()
        .map(|image| image["position"].as_i64().unwrap())
        .collect();
    assert_eq!(positions, vec![0, 1, 2]);

    // Promote the second image: positions swap, the catalog list now
    // serves IT as the product's image.
    let second_id: Uuid = images[1]["id"].as_str().unwrap().parse().unwrap();
    let response = operator
        .client
        .post(&format!(
            "/v1/merchant/products/{product_id}/images/{second_id}/cover"
        ))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let list: Value = operator
        .client
        .get(&format!("/v1/merchant/products/{product_id}/images"))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let images = list.as_array().unwrap();
    assert_eq!(images[0]["id"], second_id.to_string());
    assert_eq!(images[0]["image_url"], Value::String(urls[1].clone()));

    let catalog: Value = operator
        .client
        .get("/v1/merchant/products")
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        catalog[0]["image_url"],
        Value::String(urls[1].clone()),
        "the cover wins the product's image_url"
    );
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn gallery_delete_removes_row_and_object(pool: PgPool) {
    let app = spawn_app(pool).await;
    let operator = owner(&app, "owner@tuma.rw").await;
    let product_id = create_product_via_api(&operator).await;

    let created: Value = operator
        .client
        .post(&format!("/v1/merchant/products/{product_id}/images"))
        .bearer_auth(&operator.token)
        .multipart(png_form())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let image_id: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    let key = key_from_url(created["image_url"].as_str().unwrap()).to_string();

    let response = operator
        .client
        .delete(&format!(
            "/v1/merchant/products/{product_id}/images/{image_id}"
        ))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 204);

    let served = operator
        .client
        .get(&format!("/v1/files/{key}"))
        .send()
        .await
        .unwrap();
    assert_eq!(served.status(), 404, "the object is cleaned up");

    let list: Value = operator
        .client
        .get(&format!("/v1/merchant/products/{product_id}/images"))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn foreign_galleries_look_missing_and_managers_are_barred(pool: PgPool) {
    let app = spawn_app(pool).await;
    let aline = owner(&app, "aline@tuma.rw").await;
    let bruce = owner(&app, "bruce@tuma.rw").await;

    let alines_product = create_product_via_api(&aline).await;
    let created: Value = aline
        .client
        .post(&format!("/v1/merchant/products/{}/images", alines_product))
        .bearer_auth(&aline.token)
        .multipart(png_form())
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let image_id: Uuid = created["id"].as_str().unwrap().parse().unwrap();

    // Bruce cannot delete or promote Aline's images…
    let delete = bruce
        .client
        .delete(&format!(
            "/v1/merchant/products/{alines_product}/images/{image_id}"
        ))
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(delete.status(), 404);

    let cover = bruce
        .client
        .post(&format!(
            "/v1/merchant/products/{alines_product}/images/{image_id}/cover"
        ))
        .bearer_auth(&bruce.token)
        .send()
        .await
        .unwrap();
    assert_eq!(cover.status(), 404);

    // A store-scoped manager can dress stores but not the catalog.
    let store_id = create_store_via_api(&aline).await;
    let manager_account =
        seed_store_manager(&app.pool, "manager@tuma.rw", aline.merchant_id, store_id).await;
    let response = TestClient::new(&app.address)
        .post(&format!("/v1/merchant/products/{alines_product}/images"))
        .bearer_auth(token_for(&app, manager_account.id, 3600))
        .multipart(png_form())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn unknown_and_hostile_keys_are_plain_404s(pool: PgPool) {
    let app = spawn_app(pool).await;
    let client = TestClient::new(&app.address);

    let missing = client
        .get("/v1/files/merchants/nope/stores/nope/banner-x.jpg")
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);

    // A traversal attempt must not resolve outside the store.
    let traversal = client
        .get("/v1/files/..%2F..%2Fetc%2Fpasswd")
        .send()
        .await
        .unwrap();
    assert_eq!(traversal.status(), 404);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn gallery_is_capped_at_eight(pool: PgPool) {
    let app = spawn_app(pool).await;
    let operator = owner(&app, "owner@tuma.rw").await;
    let product_id = create_product_via_api(&operator).await;

    // Eight fit…
    for _ in 0..8 {
        let response = operator
            .client
            .post(&format!("/v1/merchant/products/{product_id}/images"))
            .bearer_auth(&operator.token)
            .multipart(png_form())
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 201);
    }

    // …the ninth is a conflict that says so…
    let response = operator
        .client
        .post(&format!("/v1/merchant/products/{product_id}/images"))
        .bearer_auth(&operator.token)
        .multipart(png_form())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 409);
    let body: Value = response.json().await.unwrap();
    assert!(
        body["message"]
            .as_str()
            .unwrap()
            .contains("gallery is full"),
        "the message names the cap: {}",
        body["message"]
    );

    // …and freeing a slot reopens the gallery.
    let list: Value = operator
        .client
        .get(&format!("/v1/merchant/products/{product_id}/images"))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let first_id: Uuid = list[0]["id"].as_str().unwrap().parse().unwrap();
    let removed = operator
        .client
        .delete(&format!(
            "/v1/merchant/products/{product_id}/images/{first_id}"
        ))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap();
    assert_eq!(removed.status(), 204);

    let response = operator
        .client
        .post(&format!("/v1/merchant/products/{product_id}/images"))
        .bearer_auth(&operator.token)
        .multipart(png_form())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 201);
}

#[sqlx::test(migrator = "MIGRATOR")]
async fn customer_menu_carries_the_full_gallery_cover_first(pool: PgPool) {
    let app = spawn_app(pool).await;
    let operator = owner(&app, "owner@tuma.rw").await;
    let store_id = create_store_via_api(&operator).await;
    let product_id = create_product_via_api(&operator).await;

    // Two images: the first uploaded is the cover, then promote the second.
    let mut uploaded: Vec<String> = Vec::new();
    for _ in 0..2 {
        let body: Value = operator
            .client
            .post(&format!("/v1/merchant/products/{product_id}/images"))
            .bearer_auth(&operator.token)
            .multipart(png_form())
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        uploaded.push(body["image_url"].as_str().unwrap().to_string());
    }
    let second_id: String = operator
        .client
        .get(&format!("/v1/merchant/products/{product_id}/images"))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()[1]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let promoted = operator
        .client
        .post(&format!(
            "/v1/merchant/products/{product_id}/images/{second_id}/cover"
        ))
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap();
    assert_eq!(promoted.status(), 204);

    // Attach to the store's assortment and open the store.
    let attached = operator
        .client
        .post_json(
            "/v1/merchant/store-products",
            json!({ "store_id": store_id, "product_id": product_id, "price": 3500 }),
        )
        .bearer_auth(&operator.token)
        .send()
        .await
        .unwrap();
    assert_eq!(attached.status(), 201);
    sqlx::query("UPDATE marketplace.stores SET is_open = true WHERE id = $1")
        .bind(store_id)
        .execute(&app.pool)
        .await
        .unwrap();

    // The customer menu carries BOTH composed URLs, promoted cover first.
    let customer = common::seed_customer(&app.pool, "+250780000009").await;
    let menu: Value = TestClient::new(&app.address)
        .get(&format!("/v1/stores/{store_id}"))
        .bearer_auth(token_for(&app, customer.account.id, 3600))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let item = &menu["products"][0];
    let images: Vec<&str> = item["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|url| url.as_str().unwrap())
        .collect();
    assert_eq!(images.len(), 2);
    assert_eq!(images[0], uploaded[1], "the promoted cover leads");
    assert_eq!(images[1], uploaded[0]);
    assert_eq!(
        item["image_url"],
        Value::String(uploaded[1].clone()),
        "the single-image field follows the cover"
    );
}
