//! 復号済みの要素を入力順に消費し、済んだページを解放する一時領域。

pub(super) const PAGE_ROWS: usize = 256;
/// 一度に復号するページの数。
pub(super) const BATCH_PAGES: usize = 64;

pub(super) struct Pending<T> {
    pages: Vec<Option<Vec<Option<T>>>>,
    len: usize,
    released: usize,
}

impl<T> Pending<T> {
    pub(super) fn with_len(len: usize) -> Self {
        Self {
            pages: (0..len.div_ceil(PAGE_ROWS)).map(|_| None).collect(),
            len,
            released: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn prepare(&mut self, index: usize) -> Option<&mut Vec<Option<T>>> {
        if index >= self.len {
            return None;
        }
        let page_index = index / PAGE_ROWS;
        let len = (self.len - page_index * PAGE_ROWS).min(PAGE_ROWS);
        let page = self.pages.get_mut(page_index)?;
        let slots = page.get_or_insert_with(|| (0..len).map(|_| None).collect());
        if slots.len() < len {
            slots.resize_with(len, || None);
        }
        Some(slots)
    }

    /// `first_page` から始まるページを置き換える。範囲の外のページは捨てる。
    pub(super) fn install(&mut self, first_page: usize, pages: Vec<Option<Vec<Option<T>>>>) {
        for (offset, page) in pages.into_iter().enumerate() {
            if let Some(slot) = first_page
                .checked_add(offset)
                .and_then(|index| self.pages.get_mut(index))
            {
                *slot = page;
            }
        }
    }

    #[cfg(test)]
    pub(super) fn push(&mut self, value: Option<T>) {
        let index = self.len;
        self.len = self.len.saturating_add(1);
        if index / PAGE_ROWS == self.pages.len() {
            self.pages.push(None);
        }
        if let Some(value) = value
            && let Some(page) = self.prepare(index)
            && let Some(slot) = page.get_mut(index % PAGE_ROWS)
        {
            *slot = Some(value);
        }
    }

    pub(super) fn get_mut(&mut self, index: usize) -> Option<&mut Option<T>> {
        if index >= self.len {
            return None;
        }
        self.pages
            .get_mut(index / PAGE_ROWS)?
            .as_mut()?
            .get_mut(index % PAGE_ROWS)
    }

    /// 昇順の入力位置を取り出し、その前に残ったページは使い終えたものとして解放する。
    pub(super) fn take_ordered(&mut self, index: usize) -> Option<T> {
        if index >= self.len {
            return None;
        }
        let page_index = index / PAGE_ROWS;
        while self.released < page_index {
            if let Some(page) = self.pages.get_mut(self.released) {
                *page = None;
            }
            self.released += 1;
        }
        let value = self.get_mut(index).and_then(Option::take);
        if index % PAGE_ROWS == PAGE_ROWS - 1 || index == self.len - 1 {
            if let Some(page) = self.pages.get_mut(page_index) {
                *page = None;
            }
            self.released = self.released.max(page_index + 1);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;

    #[test]
    fn sparse_pushes_and_updates_keep_input_positions() {
        let mut pending = Pending::with_len(0);
        let mut expected = Vec::new();
        for index in 0..PAGE_ROWS * 2 + 3 {
            let value = (index % 3 == 0).then_some(index);
            pending.push(value);
            expected.push(value);
        }
        *pending.get_mut(0).unwrap() = Some(1000);
        expected[0] = Some(1000);
        for (index, expected) in expected.into_iter().enumerate() {
            assert_eq!(pending.take_ordered(index), expected);
        }
        assert!(pending.take_ordered(usize::MAX).is_none());
    }

    struct CountDrop(Rc<Cell<usize>>);

    impl Drop for CountDrop {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn consuming_later_positions_releases_earlier_pages() {
        let dropped = Rc::new(Cell::new(0));
        let mut pending = Pending::with_len(PAGE_ROWS * 3);
        for index in [0, PAGE_ROWS + 2, PAGE_ROWS * 2] {
            pending.prepare(index).unwrap()[index % PAGE_ROWS] =
                Some(CountDrop(Rc::clone(&dropped)));
        }
        let value = pending.take_ordered(PAGE_ROWS + 2);
        assert_eq!(dropped.get(), 1);
        drop(value);
        assert_eq!(dropped.get(), 2);
        assert!(pending.take_ordered(0).is_none());
        drop(pending.take_ordered(PAGE_ROWS * 2));
        assert_eq!(dropped.get(), 3);
    }
}
