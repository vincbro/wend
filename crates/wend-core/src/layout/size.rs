use crate::number;

/// How much of the space on one axis an element takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// All of the space.
    Fill,
    /// This many cells, or all of the space when it is smaller.
    Fixed(u16),
    /// This much of the space, in percent. More than 100 is 100.
    Percent(u8),
}

impl Size {
    /// Returns how many of the `available` cells this size takes, never more than that.
    #[must_use]
    pub(super) fn resolve(self, available: u16) -> u16 {
        match self {
            Self::Fill => available,
            Self::Fixed(cells) => cells.min(available),
            Self::Percent(percent) => {
                let cells = u32::from(available) * u32::from(percent.min(100)) / 100;
                number::exact(cells)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_takes_all_of_the_space() {
        assert_eq!(Size::Fill.resolve(40), 40);
    }

    #[test]
    fn fixed_takes_its_cells_but_never_more_than_the_space() {
        assert_eq!(Size::Fixed(10).resolve(40), 10, "fits");
        assert_eq!(Size::Fixed(50).resolve(40), 40, "the space is smaller");
    }

    #[test]
    fn percent_takes_its_share_rounded_down() {
        assert_eq!(Size::Percent(50).resolve(41), 20, "half of 41");
        assert_eq!(Size::Percent(100).resolve(41), 41, "all of it");
        assert_eq!(Size::Percent(200).resolve(41), 41, "more than all is all");
        assert_eq!(Size::Percent(50).resolve(u16::MAX), 32_767, "no overflow");
    }
}
