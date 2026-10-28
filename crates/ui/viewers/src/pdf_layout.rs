pub(crate) const PAGE_GAP: f32 = 10.5;
pub(crate) const PAGE_PADDING: f32 = 14.0;

#[must_use]
pub(crate) const fn page_top(index: usize) -> f32 {
    if index == 0 { PAGE_PADDING } else { PAGE_GAP }
}

fn item_height(index: usize, page_height: f32, scale: f32) -> f32 {
    page_height.mul_add(scale, page_top(index))
}

#[must_use]
pub(crate) fn item_start(sizes: &[(f32, f32)], scale: f32, index: usize) -> f32 {
    sizes
        .iter()
        .take(index)
        .enumerate()
        .map(|(at, (_, height))| item_height(at, *height, scale))
        .sum()
}

#[must_use]
pub(crate) fn locate(sizes: &[(f32, f32)], scale: f32, y: f32) -> (usize, f32) {
    let mut top = 0.0;
    for (index, (_, height)) in sizes.iter().enumerate() {
        let bottom = top + item_height(index, *height, scale);
        if y < bottom || index + 1 == sizes.len() {
            return (index, (y - top).max(0.0));
        }
        top = bottom;
    }
    (0, 0.0)
}

#[must_use]
pub(crate) fn page_under_center(
    sizes: &[(f32, f32)],
    scale: f32,
    scroll_top: (usize, f32),
    viewport_height: f32,
) -> usize {
    let top = item_start(sizes, scale, scroll_top.0) + scroll_top.1;
    locate(sizes, scale, top + viewport_height / 2.0).0
}

#[must_use]
pub(crate) fn centered_on(
    sizes: &[(f32, f32)],
    scale: f32,
    page: usize,
    y_in_page: f32,
    viewport_height: f32,
) -> (usize, f32) {
    let target = y_in_page.mul_add(scale, item_start(sizes, scale, page) + page_top(page));
    locate(sizes, scale, (target - viewport_height / 2.0).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZES: [(f32, f32); 3] = [(600.0, 800.0), (600.0, 800.0), (600.0, 400.0)];

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn items_include_the_gap_above_each_page() {
        assert!(close(item_start(&SIZES, 1.0, 0), 0.0));
        assert!(close(item_start(&SIZES, 1.0, 1), 814.0));
        assert!(close(item_start(&SIZES, 0.5, 2), 414.0 + 410.5));
        assert_eq!(locate(&SIZES, 1.0, 10.0), (0, 10.0));
        let (item, offset) = locate(&SIZES, 1.0, 820.0);
        assert!(item == 1 && close(offset, 6.0));
        assert_eq!(locate(&SIZES, 1.0, 99_999.0).0, 2);
    }

    #[test]
    fn the_current_page_is_the_one_under_the_viewport_centre() {
        assert_eq!(page_under_center(&SIZES, 1.0, (0, 0.0), 600.0), 0);
        assert_eq!(page_under_center(&SIZES, 1.0, (0, 600.0), 600.0), 1);
        assert_eq!(page_under_center(&SIZES, 1.0, (1, 700.0), 600.0), 2);
    }

    #[test]
    fn a_match_is_scrolled_to_the_middle_of_the_viewport() {
        assert_eq!(centered_on(&SIZES, 1.0, 0, 100.0, 600.0), (0, 0.0));
        let (item, offset) = centered_on(&SIZES, 1.0, 1, 500.0, 600.0);
        assert!(item == 1 && close(offset, PAGE_GAP + 500.0 - 300.0));
        let (item, offset) = centered_on(&SIZES, 2.0, 1, 10.0, 600.0);
        assert!(item == 0 && close(offset, 1614.0 + PAGE_GAP + 20.0 - 300.0));
    }
}
