pub mod section {
    use num_traits::PrimInt;

    #[inline]
    pub fn block_to_section<T: PrimInt>(coord: T) -> T {
        coord >> 4
    }

    #[must_use]
    pub fn get_offset_pos(chunk_coord: i32, offset: i32) -> i32 {
        section_to_block(chunk_coord) + offset
    }

    #[inline]
    pub fn section_to_block<T: PrimInt>(coord: T) -> T {
        coord << 4
    }
}

pub mod biome {
    use num_traits::PrimInt;

    #[inline]
    pub fn from_block<T: PrimInt>(coord: T) -> T {
        coord >> 2
    }

    #[inline]
    pub fn to_block<T: PrimInt>(coord: T) -> T {
        coord << 2
    }

    #[inline]
    pub fn from_chunk<T: PrimInt>(coord: T) -> T {
        coord << 2
    }

    #[inline]
    pub fn to_chunk<T: PrimInt>(coord: T) -> T {
        coord >> 2
    }
}

#[derive(PartialEq, Eq)]
pub enum Direction {
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
    NorthWest,
}
