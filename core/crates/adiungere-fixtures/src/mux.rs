//! A small box writer, enough to lay out a recording the way a camera does.
//!
//! This is a fixture builder, not the product's writer. It synthesises a movie from scratch for the tests
//! to read; the product's remuxer, which copies ranges from a real recording, arrives with its own
//! milestone and is measured against files this builder produces.

/// Builds one box with a 32-bit size.
pub(crate) fn boxed(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let size = u32::try_from(payload.len().saturating_add(8)).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(payload.len().saturating_add(8));
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(payload);
    out
}

/// Builds one full box: version, flags, then the payload.
pub(crate) fn full_box(kind: [u8; 4], version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(payload.len().saturating_add(4));
    body.push(version);
    body.extend_from_slice(flags.to_be_bytes().get(1..4).unwrap_or(&[0, 0, 0]));
    body.extend_from_slice(payload);
    boxed(kind, &body)
}

/// Builds a container from its children.
pub(crate) fn container(kind: [u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = children.iter().flatten().copied().collect();
    boxed(kind, &payload)
}

/// A header for a box whose payload is written elsewhere, with a 64-bit size.
pub(crate) fn large_header(kind: [u8; 4], total_size: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&kind);
    out.extend_from_slice(&total_size.to_be_bytes());
    out
}

/// A header for a box whose payload is written elsewhere, with a 32-bit size.
pub(crate) fn header(kind: [u8; 4], total_size: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(8);
    out.extend_from_slice(&total_size.to_be_bytes());
    out.extend_from_slice(&kind);
    out
}

/// A builder for the fields of one box, in order.
#[derive(Debug, Default)]
pub(crate) struct Fields(Vec<u8>);

impl Fields {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn u8(mut self, value: u8) -> Self {
        self.0.push(value);
        self
    }

    pub(crate) fn u16(mut self, value: u16) -> Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub(crate) fn i16(mut self, value: i16) -> Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub(crate) fn u32(mut self, value: u32) -> Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub(crate) fn i32(mut self, value: i32) -> Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub(crate) fn u64(mut self, value: u64) -> Self {
        self.0.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub(crate) fn bytes(mut self, value: &[u8]) -> Self {
        self.0.extend_from_slice(value);
        self
    }

    pub(crate) fn zeros(mut self, count: usize) -> Self {
        self.0.extend(std::iter::repeat_n(0, count));
        self
    }

    /// The identity matrix a track and a movie header carry.
    pub(crate) fn unity_matrix(self) -> Self {
        self.u32(0x0001_0000)
            .u32(0)
            .u32(0)
            .u32(0)
            .u32(0x0001_0000)
            .u32(0)
            .u32(0)
            .u32(0)
            .u32(0x4000_0000)
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.0
    }
}

/// A descriptor with the expandable length the elementary stream descriptor box uses.
pub(crate) fn descriptor(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let mut length = payload.len();
    let mut encoded = Vec::new();
    loop {
        let byte = u8::try_from(length & 0x7f).unwrap_or(0);
        encoded.push(byte);
        length >>= 7;
        if length == 0 {
            break;
        }
    }
    encoded.reverse();
    for (position, byte) in encoded.iter().enumerate() {
        let more = position.saturating_add(1) < encoded.len();
        out.push(if more { byte | 0x80 } else { *byte });
    }
    out.extend_from_slice(payload);
    out
}

#[cfg(test)]
mod tests {
    use super::{boxed, descriptor, full_box};

    #[test]
    fn a_box_carries_its_size_type_and_payload() {
        // Act
        let built = boxed(*b"free", &[1, 2, 3]);

        // Assert
        assert_eq!(built, vec![0, 0, 0, 11, b'f', b'r', b'e', b'e', 1, 2, 3]);
    }

    #[test]
    fn a_full_box_places_version_and_three_flag_bytes_first() {
        // Act
        let built = full_box(*b"tkhd", 1, 0x0000_0007, &[]);

        // Assert
        assert_eq!(built.get(8..12), Some(&[1, 0, 0, 7][..]));
    }

    #[test]
    fn a_descriptor_length_is_expandable() {
        // Act
        let short = descriptor(3, &[0; 5]);
        let long = descriptor(3, &[0; 200]);

        // Assert
        assert_eq!(short.get(..2), Some(&[3, 5][..]));
        assert_eq!(long.get(..3), Some(&[3, 0x81, 0x48][..]));
    }
}
