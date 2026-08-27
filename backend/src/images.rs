use sha2::{Digest, Sha256};

pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageType {
    Png,
    Jpeg,
    Webp,
}

impl ImageType {
    pub fn extension(self) -> &'static str {
        match self {
            ImageType::Png => "png",
            ImageType::Jpeg => "jpg",
            ImageType::Webp => "webp",
        }
    }
}

pub fn sniff(bytes: &[u8]) -> Option<ImageType> {
    const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    if bytes.starts_with(PNG_SIGNATURE) {
        return Some(ImageType::Png);
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageType::Jpeg);
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(ImageType::Webp);
    }
    None
}

pub fn stored_name(bytes: &[u8], image_type: ImageType) -> String {
    let digest = Sha256::digest(bytes);
    let mut stem = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write;
        let _ = write!(stem, "{byte:02x}");
    }
    format!("{stem}.{}", image_type.extension())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_signature(signature: &[u8], filler: usize) -> Vec<u8> {
        let mut bytes = signature.to_vec();
        bytes.resize(signature.len() + filler, 0xAB);
        bytes
    }

    const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];

    fn riff_container(tag: &[u8; 4]) -> Vec<u8> {
        let mut bytes = b"RIFF".to_vec();
        bytes.extend_from_slice(&[0x24, 0x00, 0x00, 0x00]);
        bytes.extend_from_slice(tag);
        bytes
    }

    #[test]
    fn sniffs_the_three_accepted_types() {
        assert_eq!(sniff(&with_signature(PNG_SIGNATURE, 32)), Some(ImageType::Png));
        assert_eq!(sniff(&with_signature(JPEG_SIGNATURE, 32)), Some(ImageType::Jpeg));
        assert_eq!(sniff(&riff_container(b"WEBP")), Some(ImageType::Webp));
    }

    #[test]
    fn rejects_a_file_that_is_not_an_image() {
        assert_eq!(sniff(b"just some notes about k-means"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn rejects_a_truncated_signature() {
        assert_eq!(sniff(&PNG_SIGNATURE[..7]), None);
    }

    #[test]
    fn rejects_a_riff_container_that_is_not_webp() {
        assert_eq!(sniff(&riff_container(b"WAVE")), None);
        assert_eq!(sniff(b"RIFF\x24\x00\x00\x00"), None);
    }

    #[test]
    fn a_name_is_sixteen_hex_characters_plus_the_sniffed_extension() {
        let name = stored_name(&with_signature(PNG_SIGNATURE, 10), ImageType::Png);
        let (stem, extension) = name.rsplit_once('.').expect("name has an extension");
        assert_eq!(extension, "png");
        assert_eq!(stem.len(), 16);
        assert!(
            stem.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')),
            "stem must be lowercase hex, got {stem}",
        );
    }

    #[test]
    fn identical_bytes_get_identical_names() {
        let bytes = with_signature(PNG_SIGNATURE, 64);
        assert_eq!(
            stored_name(&bytes, ImageType::Png),
            stored_name(&bytes.clone(), ImageType::Png),
        );
    }

    #[test]
    fn different_bytes_get_different_names() {
        let shorter = with_signature(PNG_SIGNATURE, 64);
        let longer = with_signature(PNG_SIGNATURE, 65);
        assert_ne!(
            stored_name(&shorter, ImageType::Png),
            stored_name(&longer, ImageType::Png),
        );
    }

    #[test]
    fn the_extension_follows_the_type_not_the_bytes_length() {
        let bytes = with_signature(JPEG_SIGNATURE, 8);
        assert!(stored_name(&bytes, ImageType::Jpeg).ends_with(".jpg"));
        assert!(stored_name(&bytes, ImageType::Webp).ends_with(".webp"));
    }
}
