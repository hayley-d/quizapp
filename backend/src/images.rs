//! Upload policy: what counts as an image, and what it is called on disk.
//!
//! Free of axum and sqlx on purpose, so the rules can be unit-tested without
//! a request or a database. `routes::images` is the thin HTTP wrapper.

use sha2::{Digest, Sha256};

/// 5 MiB. A diagram cropped out of a lecture slide is tens of kilobytes;
/// anything approaching this is a phone photo pasted in by accident.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

/// The types the app accepts, identified by signature rather than by the
/// client's `Content-Type` or the uploaded filename. Both of those are
/// caller-controlled, and nothing downstream re-checks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageType {
    Png,
    Jpeg,
    Webp,
}

impl ImageType {
    /// The extension written to disk. This set must stay in step with the
    /// `image_path` guard in `routes::cards::is_uploaded_image_path` — that
    /// guard rejects any path this function could not have produced.
    pub fn extension(self) -> &'static str {
        match self {
            ImageType::Png => "png",
            ImageType::Jpeg => "jpg",
            ImageType::Webp => "webp",
        }
    }
}

/// Identifies an image by its leading bytes.
///
/// A signature check, not a decode: it proves the file claims to be a PNG,
/// JPEG or WebP, not that the remainder is well-formed. That is the right
/// depth here, because the app never decodes these files — it writes them to
/// disk and lets the browser render them. What it does buy is that a `.png`
/// full of something else cannot be stored under a `.png` name.
pub fn sniff(bytes: &[u8]) -> Option<ImageType> {
    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    if bytes.starts_with(PNG) {
        return Some(ImageType::Png);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageType::Jpeg);
    }
    // `RIFF` <4-byte little-endian size> `WEBP`. WAV and AVI are also RIFF
    // containers, so the tag at offset 8 is the part that matters.
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(ImageType::Webp);
    }
    None
}

/// Content-addressed filename: the first 8 bytes of the SHA-256 of the
/// contents as hex, plus the sniffed extension.
///
/// No RNG dependency, no collision bookkeeping, and uploading the same
/// diagram twice reuses the file already written. 64 bits of hash over a
/// personal card deck will not collide.
pub fn stored_name(bytes: &[u8], kind: ImageType) -> String {
    let digest = Sha256::digest(bytes);
    let mut stem = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write;
        let _ = write!(stem, "{byte:02x}");
    }
    format!("{stem}.{}", kind.extension())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file with a valid signature and `filler` bytes of arbitrary payload.
    /// The sniffer reads signatures, not pixels, so this is exactly as much
    /// file as the code under test can distinguish.
    fn with_signature(sig: &[u8], filler: usize) -> Vec<u8> {
        let mut v = sig.to_vec();
        v.resize(sig.len() + filler, 0xAB);
        v
    }

    const PNG_SIG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    const JPEG_SIG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];

    fn webp(tag: &[u8; 4]) -> Vec<u8> {
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&[0x24, 0x00, 0x00, 0x00]); // little-endian size
        v.extend_from_slice(tag);
        v
    }

    #[test]
    fn sniffs_the_three_accepted_types() {
        assert_eq!(sniff(&with_signature(PNG_SIG, 32)), Some(ImageType::Png));
        assert_eq!(sniff(&with_signature(JPEG_SIG, 32)), Some(ImageType::Jpeg));
        assert_eq!(sniff(&webp(b"WEBP")), Some(ImageType::Webp));
    }

    #[test]
    fn rejects_a_file_that_is_not_an_image() {
        assert_eq!(sniff(b"just some notes about k-means"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn rejects_a_truncated_signature() {
        // Seven of the PNG signature's eight bytes. A prefix check that was
        // one byte short would accept this.
        assert_eq!(sniff(&PNG_SIG[..7]), None);
    }

    #[test]
    fn rejects_a_riff_container_that_is_not_webp() {
        // A WAV file also starts "RIFF". Checking only the container would
        // let it through and write it out as `.webp`.
        assert_eq!(sniff(&webp(b"WAVE")), None);
        // RIFF with nothing after the size field is not long enough to judge.
        assert_eq!(sniff(b"RIFF\x24\x00\x00\x00"), None);
    }

    #[test]
    fn a_name_is_sixteen_hex_characters_plus_the_sniffed_extension() {
        let name = stored_name(&with_signature(PNG_SIG, 10), ImageType::Png);
        let (stem, ext) = name.rsplit_once('.').expect("name has an extension");
        assert_eq!(ext, "png");
        assert_eq!(stem.len(), 16);
        assert!(
            stem.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f')),
            "stem must be lowercase hex, got {stem}",
        );
    }

    #[test]
    fn identical_bytes_get_identical_names() {
        let a = with_signature(PNG_SIG, 64);
        assert_eq!(stored_name(&a, ImageType::Png), stored_name(&a.clone(), ImageType::Png));
    }

    #[test]
    fn different_bytes_get_different_names() {
        let a = with_signature(PNG_SIG, 64);
        let b = with_signature(PNG_SIG, 65);
        assert_ne!(stored_name(&a, ImageType::Png), stored_name(&b, ImageType::Png));
    }

    #[test]
    fn the_extension_follows_the_type_not_the_bytes_length() {
        let bytes = with_signature(JPEG_SIG, 8);
        assert!(stored_name(&bytes, ImageType::Jpeg).ends_with(".jpg"));
        assert!(stored_name(&bytes, ImageType::Webp).ends_with(".webp"));
    }
}
