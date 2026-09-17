//! Boxes as ranges.

use serde::Serialize;

use crate::fourcc::FourCc;

/// One box: where it is, how large it is, and which boxes it holds, if it was enumerated.
///
/// A range is kept for every box, known or not. `children` is filled only for the containers the reader
/// enumerates; for everything else it is empty and the payload is whatever lies between the header and the
/// end of the box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoxRange {
    /// The type code from the header.
    pub kind: FourCc,
    /// The absolute offset of the box's first byte, which is the start of its header.
    pub offset: u64,
    /// The size of the whole box, header included. Resolved from a 64-bit size or from "to the end of the
    /// file" where the header used either form.
    pub size: u64,
    /// The header length: 8, or 16 when the size needed 64 bits.
    pub header: u8,
    /// How many bytes of the box's own fields follow the header before its children begin. Zero for a pure
    /// container; the version, flags and entry count of a sample description; the fixed fields of a sample
    /// entry.
    pub fields: u32,
    /// The children, in file order, for a container the reader enumerated.
    pub children: Vec<BoxRange>,
    /// Whether the declared size ran past the parent and was clipped to it. Only tolerated where a vendor
    /// writes what it likes, which is under the user-data box and inside a sample entry; elsewhere an
    /// overrun is an error.
    pub clipped: bool,
}

impl BoxRange {
    /// The offset of the first payload byte.
    #[must_use]
    pub fn payload_offset(&self) -> u64 {
        self.offset.saturating_add(u64::from(self.header))
    }

    /// The payload length, which is the size less the header.
    #[must_use]
    pub fn payload_size(&self) -> u64 {
        self.size.saturating_sub(u64::from(self.header))
    }

    /// The offset at which the children begin: after the header and the box's own fields.
    #[must_use]
    pub fn children_offset(&self) -> u64 {
        self.payload_offset().saturating_add(u64::from(self.fields))
    }

    /// One past the last byte.
    #[must_use]
    pub fn end(&self) -> u64 {
        self.offset.saturating_add(self.size)
    }

    /// The first child of a type, if any.
    #[must_use]
    pub fn child(&self, kind: &[u8; 4]) -> Option<&BoxRange> {
        self.children.iter().find(|child| child.kind == kind)
    }

    /// Every child of a type, in order.
    pub fn children_of(&self, kind: &[u8; 4]) -> impl Iterator<Item = &BoxRange> {
        self.children.iter().filter(move |child| child.kind == kind)
    }

    /// Follows a path of types from this box, taking the first match at each step.
    #[must_use]
    pub fn descend(&self, path: &[&[u8; 4]]) -> Option<&BoxRange> {
        path.iter().try_fold(self, |current, kind| current.child(kind))
    }

    /// Whether the children, where there are any, tile the payload exactly: no gap, no overlap, in order.
    ///
    /// This is the property that makes "re-emit the ranges" reproduce the input, and it is what the round
    /// trip guarantee measures.
    #[must_use]
    pub fn children_tile_payload(&self) -> bool {
        if self.children.is_empty() {
            return true;
        }

        let mut cursor = self.children_offset();

        for child in &self.children {
            if child.offset != cursor || !child.children_tile_payload() {
                return false;
            }
            cursor = child.end();
        }

        cursor == self.end()
    }

    /// Visits this box and every descendant, depth first, with the path of types that leads to each.
    pub fn walk<'a>(&'a self, prefix: &str, into: &mut Vec<(String, &'a BoxRange)>) {
        let path = if prefix.is_empty() {
            self.kind.to_string()
        } else {
            format!("{prefix}/{}", self.kind)
        };

        into.push((path.clone(), self));

        for child in &self.children {
            child.walk(&path, into);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BoxRange;
    use crate::fourcc::FourCc;

    fn leaf(kind: [u8; 4], offset: u64, size: u64) -> BoxRange {
        BoxRange {
            kind: FourCc(kind),
            offset,
            size,
            header: 8,
            fields: 0,
            children: Vec::new(),
            clipped: false,
        }
    }

    #[test]
    fn the_payload_is_the_size_less_the_header() {
        // Arrange
        let range = BoxRange {
            header: 16,
            ..leaf(*b"mdat", 100, 1000)
        };

        // Act and assert
        assert_eq!(range.payload_offset(), 116);
        assert_eq!(range.payload_size(), 984);
        assert_eq!(range.end(), 1100);
        assert_eq!(leaf(*b"free", 0, 8).payload_size(), 0);
    }

    #[test]
    fn a_box_with_its_own_fields_places_its_children_after_them() {
        // Arrange
        let stsd = BoxRange {
            fields: 8,
            children: vec![leaf(*b"avc1", 16, 14)],
            ..leaf(*b"stsd", 0, 30)
        };

        // Act and assert
        assert_eq!(stsd.children_offset(), 16);
        assert!(stsd.children_tile_payload());
    }

    #[test]
    fn children_that_abut_from_the_payload_to_the_end_tile_it() {
        // Arrange
        let parent = BoxRange {
            children: vec![leaf(*b"aaaa", 8, 10), leaf(*b"bbbb", 18, 12)],
            ..leaf(*b"moov", 0, 30)
        };

        // Act and assert
        assert!(parent.children_tile_payload());
    }

    #[test]
    fn a_gap_between_children_breaks_the_tiling() {
        // Arrange
        let parent = BoxRange {
            children: vec![leaf(*b"aaaa", 8, 10), leaf(*b"bbbb", 20, 10)],
            ..leaf(*b"moov", 0, 30)
        };

        // Act and assert
        assert!(!parent.children_tile_payload());
    }

    #[test]
    fn a_child_that_stops_short_of_the_end_breaks_the_tiling() {
        // Arrange
        let parent = BoxRange {
            children: vec![leaf(*b"aaaa", 8, 10)],
            ..leaf(*b"moov", 0, 30)
        };

        // Act and assert
        assert!(!parent.children_tile_payload());
    }

    #[test]
    fn a_path_is_followed_through_the_first_match_at_each_step() {
        // Arrange
        let stbl = BoxRange {
            children: vec![leaf(*b"stsd", 40, 8)],
            ..leaf(*b"stbl", 32, 16)
        };
        let trak = BoxRange {
            children: vec![stbl],
            ..leaf(*b"trak", 24, 24)
        };
        let moov = BoxRange {
            children: vec![leaf(*b"mvhd", 8, 16), trak],
            ..leaf(*b"moov", 0, 48)
        };

        // Act
        let found = moov.descend(&[b"trak", b"stbl", b"stsd"]);

        // Assert
        assert_eq!(found.map(|range| range.offset), Some(40));
        assert!(moov.descend(&[b"trak", b"udta"]).is_none());
    }

    #[test]
    fn walking_yields_every_box_with_its_path() {
        // Arrange
        let moov = BoxRange {
            children: vec![BoxRange {
                children: vec![leaf(*b"zvnd", 16, 8)],
                ..leaf(*b"udta", 8, 16)
            }],
            ..leaf(*b"moov", 0, 24)
        };
        let mut visited = Vec::new();

        // Act
        moov.walk("", &mut visited);

        // Assert
        let paths: Vec<&str> = visited.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(paths, vec!["moov", "moov/udta", "moov/udta/zvnd"]);
    }
}
