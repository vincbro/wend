//! Conversions between integer types.

use std::{any, fmt::Debug};

use az::{CheckedCast, SaturatingCast};

/// Converts a number whose value is known to fit `Destination`, such as a position inside an
/// `Area` or a byte offset in a buffer that `buffer` already limited.
///
/// Debug builds panic when it doesn't fit, so a broken assumption shows up in tests.
/// Release builds keep the nearest value that does, because a wrong cell on screen is
/// better than losing the user's unsaved work.
///
/// # Panics
///
/// In debug builds, if `value` doesn't fit in `Destination`.
#[must_use]
#[track_caller]
pub(crate) fn exact<Source, Destination>(value: Source) -> Destination
where
    Source: CheckedCast<Destination> + SaturatingCast<Destination> + Copy + Debug,
{
    let checked = value.checked_cast();
    debug_assert!(
        checked.is_some(),
        "{value:?} doesn't fit in a {}",
        any::type_name::<Destination>()
    );
    checked.unwrap_or_else(|| value.saturating_cast())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_keeps_a_value_that_fits() {
        assert_eq!(exact::<usize, u16>(65_535), u16::MAX);
        assert_eq!(exact::<u8, usize>(7), 7);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "doesn't fit in a u16")]
    fn exact_panics_in_debug_when_the_value_does_not_fit() {
        let _ = exact::<usize, u16>(65_536);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn exact_saturates_in_release_when_the_value_does_not_fit() {
        assert_eq!(exact::<usize, u16>(65_536), u16::MAX);
        assert_eq!(exact::<i32, u8>(-1), 0);
    }
}
