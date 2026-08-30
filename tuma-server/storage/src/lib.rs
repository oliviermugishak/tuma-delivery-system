//! Object storage for images (slice U1): validation, normalization, keys,
//! and the backend-agnostic handle on the store.
//!
//! The write path is proxy-style: the API accepts the bytes, validates
//! them by content (magic numbers, never the client's declared
//! Content-Type), normalizes them once (EXIF orientation applied, fit
//! within 1600px, re-encoded JPEG q85), and writes to the configured
//! backend — local disk in dev, in-memory in tests, S3-compatible
//! (Cloudflare R2) in production. One code path for all three.
//!
//! The read path is where performance lives: keys are content-UUIDs that
//! are never overwritten, so every object is `Cache-Control: immutable`,
//! and in production `public_base_url` points at the R2 public URL with
//! Cloudflare CDN in front of it. Clients only ever see URLs.

use bytes::Bytes;
use image::DynamicImage;
use image::ImageDecoder;
use image::ImageFormat;
use image::ImageReader;
use image::codecs::jpeg::JpegEncoder;
use object_store::ObjectStore;
use object_store::path::Path as ObjectPath;
use secrecy::ExposeSecret;
use std::io::Cursor;
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

use app_config::StorageConfig;

/// Hard cap accepted from the wire, before any decoding. Multipart routes
/// raise axum's default body limit a little above this (envelope overhead).
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
/// Longest edge after normalization. Feed cards render thumbnails; the
/// original pixel size of a phone photo buys nothing on mobile data.
const MAX_EDGE_PX: u32 = 1600;
const JPEG_QUALITY: u8 = 85;

#[derive(Debug, Error)]
pub enum StorageError {
    /// Not an image at all, or an image type outside the allow-list.
    #[error("that file is not a JPEG, PNG, or WebP image")]
    NotAnImage,
    #[error("images can be at most 5 MB")]
    TooLarge,
    #[error("the image could not be processed")]
    Decode(#[source] image::ImageError),
    #[error("the image could not be stored")]
    Store(#[source] object_store::Error),
    /// Backend construction failed (bad bucket, unreachable config shape).
    #[error("storage is not configured correctly: {0}")]
    Configuration(String),
}

/// The configured object store plus the base URL clients should read it
/// from. Lives in `AppState`; `public_base_url` is per-environment config
/// (an empty value means the API's own `/api/v1/files/…` route).
#[derive(Clone)]
pub struct StorageService {
    pub store: Arc<dyn ObjectStore>,
    pub public_base_url: String,
}

impl std::fmt::Debug for StorageService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageService")
            .field("public_base_url", &self.public_base_url)
            .finish_non_exhaustive()
    }
}

impl StorageService {
    /// The URL a client loads an object from. With an empty base the
    /// relative file route is returned (same-origin consumers only).
    pub fn public_url(&self, key: &str) -> String {
        public_url(&self.public_base_url, key)
    }
}

/// Build the backend from config. Called once at startup (and per test).
pub fn build_service(config: &StorageConfig) -> Result<StorageService, StorageError> {
    let store: Arc<dyn ObjectStore> = match config.backend {
        app_config::StorageBackend::Local => {
            std::fs::create_dir_all(&config.local.root)
                .map_err(|error| StorageError::Configuration(error.to_string()))?;
            let fs = object_store::local::LocalFileSystem::new_with_prefix(&config.local.root)
                .map_err(|error| StorageError::Configuration(error.to_string()))?;
            Arc::new(fs)
        }
        app_config::StorageBackend::Memory => Arc::new(object_store::memory::InMemory::new()),
        app_config::StorageBackend::S3 => {
            if config.s3.bucket.is_empty() {
                return Err(StorageError::Configuration(
                    "s3 backend selected but storage.s3.bucket is empty".into(),
                ));
            }
            let mut builder = object_store::aws::AmazonS3Builder::new()
                .with_bucket_name(&config.s3.bucket)
                .with_region(&config.s3.region)
                .with_access_key_id(&config.s3.access_key_id)
                .with_secret_access_key(config.s3.secret_access_key.expose_secret());
            if !config.s3.endpoint.is_empty() {
                builder = builder.with_endpoint(&config.s3.endpoint);
            }
            let s3 = builder
                .build()
                .map_err(|error| StorageError::Configuration(error.to_string()))?;
            Arc::new(s3)
        }
    };
    Ok(StorageService {
        store,
        public_base_url: config.public_base_url.trim_end_matches('/').to_string(),
    })
}

/// The full URL for a key against a base. Shared so the file route and
/// tests agree on the shape.
pub fn public_url(base: &str, key: &str) -> String {
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        format!("/api/v1/files/{key}")
    } else {
        format!("{base}/{key}")
    }
}

/// The URL an API response should render for an entity image: a storage
/// key (banner / gallery cover) wins, composed against the base; else the
/// legacy passthrough URL (external or placeholder) survives untouched;
/// else nothing.
pub fn resolve_image_url(base: &str, key: Option<&str>, legacy: Option<&str>) -> Option<String> {
    match key {
        Some(key) => Some(public_url(base, key)),
        None => legacy.map(str::to_string),
    }
}

/// Deterministic, collision-safe keys — never the client's filename.
/// Keys are immutable: an upload creates a new object, a replace writes a
/// NEW key and retires the old one, so reads can cache forever.
pub fn banner_key(merchant_id: Uuid, store_id: Uuid) -> String {
    format!(
        "merchants/{merchant_id}/stores/{store_id}/banner-{}.jpg",
        Uuid::new_v4()
    )
}

pub fn product_image_key(merchant_id: Uuid, product_id: Uuid) -> String {
    format!(
        "merchants/{merchant_id}/products/{product_id}/images/{}.jpg",
        Uuid::new_v4()
    )
}

/// Validate the uploaded bytes and normalize them: EXIF orientation
/// applied, downscaled to fit [`MAX_EDGE_PX`] on the long edge (small
/// images pass through), re-encoded JPEG q85. Every stored object is a
/// predictable `.jpg` — the key's extension is the content type.
///
/// Named consequences (founder-approved): JPEG-only output (food photos,
/// no transparency), EXIF stripped — including GPS tags, a privacy plus.
pub fn normalize_image(bytes: Bytes) -> Result<Bytes, StorageError> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(StorageError::TooLarge);
    }
    let reader = ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|_| StorageError::NotAnImage)?;
    let format = reader.format().ok_or(StorageError::NotAnImage)?;
    if !matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP
    ) {
        return Err(StorageError::NotAnImage);
    }

    // Everything up to a decoded image is the CLIENT's fault — fake
    // magics, truncated files — so it maps to 415, never 500. Only an
    // encode failure (a bug: the input already decoded) stays internal.
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| StorageError::NotAnImage)?;
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(|_| StorageError::NotAnImage)?;
    image.apply_orientation(orientation);
    if image.width() > MAX_EDGE_PX || image.height() > MAX_EDGE_PX {
        image = image.resize(
            MAX_EDGE_PX,
            MAX_EDGE_PX,
            image::imageops::FilterType::Lanczos3,
        );
    }

    let mut out = Vec::with_capacity(bytes.len() / 4);
    image
        .write_with_encoder(JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY))
        .map_err(StorageError::Decode)?;
    Ok(Bytes::from(out))
}

/// Put bytes at a key. Content type is a read-time concern (extension →
/// mime), so the write is payload-only.
pub async fn put_image(
    store: &Arc<dyn ObjectStore>,
    key: &str,
    bytes: Bytes,
) -> Result<(), StorageError> {
    let path = object_path(key)?;
    store
        .put(&path, object_store::PutPayload::from_bytes(bytes))
        .await
        .map_err(StorageError::Store)?;
    Ok(())
}

/// Best-effort delete — used for replaces and for cleaning up after a
/// failed DB write. Orphaned objects are cheaper than failed uploads;
/// a garbage-collector pass can sweep strays later if one ever matters.
pub async fn delete_best_effort(store: &Arc<dyn ObjectStore>, key: &str) {
    if let Ok(path) = ObjectPath::parse(key) {
        let _ = store.delete(&path).await;
    }
}

/// Whether an object-store error is a plain miss (no object at the key).
/// Keeps `object_store` out of the API crate's manifest — handlers match
/// on this instead of naming the upstream error type.
pub fn is_not_found(error: &object_store::Error) -> bool {
    matches!(error, object_store::Error::NotFound { .. })
}

/// Parse a key into an object_store path. Server-generated keys always
/// parse; the typed error keeps externally-supplied keys (the file route)
/// from ever reaching the backend with garbage.
pub fn object_path(key: &str) -> Result<ObjectPath, StorageError> {
    ObjectPath::parse(key).map_err(|_| StorageError::NotAnImage)
}

/// Content type for a stored object, from its key's extension. Keys we
/// generate are always `.jpg`; unknown extensions fall back to octet-stream.
pub fn content_type_for(key: &str) -> &'static str {
    let extension = key.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn png_bytes(width: u32, height: u32) -> Bytes {
        let image = RgbaImage::from_pixel(width, height, image::Rgba([255, 0, 0, 255]));
        let mut buffer = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut buffer), ImageFormat::Png)
            .expect("test PNG encodes");
        Bytes::from(buffer)
    }

    #[test]
    fn keys_are_unique_shaped_and_jpeg() {
        let merchant = Uuid::new_v4();
        let (a, b) = (
            banner_key(merchant, Uuid::new_v4()),
            banner_key(merchant, Uuid::new_v4()),
        );
        assert_ne!(a, b);
        assert!(a.starts_with(&format!("merchants/{merchant}/stores/")));
        assert!(a.ends_with("banner.jpg") || a.contains("/banner-"));
        let image = product_image_key(merchant, Uuid::new_v4());
        assert!(
            image.contains("/products/") && image.contains("/images/") && image.ends_with(".jpg")
        );
    }

    #[test]
    fn public_url_composes_against_the_base_or_the_file_route() {
        assert_eq!(
            public_url("https://cdn.tuma.rw", "merchants/m/x.jpg"),
            "https://cdn.tuma.rw/merchants/m/x.jpg"
        );
        assert_eq!(
            public_url("", "merchants/m/x.jpg"),
            "/api/v1/files/merchants/m/x.jpg"
        );
        assert_eq!(
            public_url("https://cdn.tuma.rw/", "merchants/m/x.jpg"),
            "https://cdn.tuma.rw/merchants/m/x.jpg",
            "a trailing slash on the base must not double up"
        );
    }

    #[test]
    fn resolve_prefers_the_key_and_falls_through_to_the_legacy_url() {
        assert_eq!(
            resolve_image_url("", Some("k.jpg"), Some("https://legacy/img")),
            Some("/api/v1/files/k.jpg".into())
        );
        assert_eq!(
            resolve_image_url("", None, Some("https://legacy/img")),
            Some("https://legacy/img".into())
        );
        assert_eq!(resolve_image_url("", None, None), None);
    }

    #[test]
    fn normalization_turns_png_into_jpeg_and_keeps_small_sizes() {
        let png = png_bytes(64, 32);
        let jpeg = normalize_image(png).expect("a real PNG normalizes");
        assert_eq!(&jpeg[..2], b"\xff\xd8", "output is JPEG");
        let decoded = ImageReader::new(Cursor::new(&jpeg[..]))
            .with_guessed_format()
            .expect("guesses")
            .decode()
            .expect("output decodes");
        assert_eq!(decoded.width(), 64);
        assert_eq!(decoded.height(), 32);
    }

    #[test]
    fn oversized_images_are_capped_at_the_long_edge() {
        let jpeg = normalize_image(png_bytes(2000, 1000)).expect("normalizes");
        let decoded = ImageReader::new(Cursor::new(&jpeg[..]))
            .with_guessed_format()
            .expect("guesses")
            .decode()
            .expect("output decodes");
        assert!(decoded.width() <= 1600 && decoded.height() <= 1600);
        assert_eq!(decoded.height(), 800, "aspect ratio preserved");
    }

    #[test]
    fn garbage_and_wrong_types_are_rejected_as_media_errors() {
        assert!(matches!(
            normalize_image(Bytes::from_static(b"definitely not an image")),
            Err(StorageError::NotAnImage)
        ));
        // JPEG magic bytes followed by garbage: decodes to nothing.
        let fake = Bytes::from_static(b"\xff\xd8\xff\xe0not really a jpeg");
        assert!(matches!(
            normalize_image(fake),
            Err(StorageError::NotAnImage)
        ));
        assert!(matches!(
            normalize_image(Bytes::from_static(&[0u8; 16])),
            Err(StorageError::NotAnImage)
        ));
    }

    #[test]
    fn oversize_rejects_before_any_decoding() {
        let too_big = Bytes::from(vec![0u8; MAX_IMAGE_BYTES + 1]);
        assert!(matches!(
            normalize_image(too_big),
            Err(StorageError::TooLarge)
        ));
    }

    #[test]
    fn content_type_follows_the_extension() {
        assert_eq!(content_type_for("a/b/banner-1.jpg"), "image/jpeg");
        assert_eq!(content_type_for("x.png"), "image/png");
        assert_eq!(content_type_for("mystery"), "application/octet-stream");
    }

    #[tokio::test]
    async fn the_memory_backend_round_trips() {
        let service = build_service(&app_config::StorageConfig {
            backend: app_config::StorageBackend::Memory,
            ..Default::default()
        })
        .expect("memory backend builds");
        let key = product_image_key(Uuid::new_v4(), Uuid::new_v4());
        put_image(&service.store, &key, Bytes::from_static(b"bytes"))
            .await
            .expect("put");
        let got = service
            .store
            .get(&object_path(&key).expect("parses"))
            .await
            .expect("get");
        assert_eq!(
            got.bytes().await.expect("body"),
            Bytes::from_static(b"bytes")
        );
        delete_best_effort(&service.store, &key).await;
        assert!(
            service
                .store
                .get(&object_path(&key).expect("parses"))
                .await
                .is_err()
        );
    }
}
