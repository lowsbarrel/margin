#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Steps {
    current: usize,
    count: usize,
}

impl Steps {
    pub(crate) const fn new(count: usize) -> Self {
        Self { current: 0, count }
    }

    pub(crate) const fn current(self) -> usize {
        self.current
    }

    pub(crate) const fn count(self) -> usize {
        self.count
    }

    pub(crate) const fn is_first(self) -> bool {
        self.current == 0
    }

    pub(crate) const fn is_last(self) -> bool {
        self.current + 1 == self.count
    }

    pub(crate) const fn go_to(&mut self, step: usize) -> bool {
        if step < self.count {
            self.current = step;
            true
        } else {
            false
        }
    }

    pub(crate) const fn next(&mut self) -> bool {
        self.go_to(self.current + 1)
    }

    pub(crate) const fn previous(&mut self) -> bool {
        match self.current.checked_sub(1) {
            Some(step) => self.go_to(step),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walks_forward_and_back_within_bounds() {
        let mut steps = Steps::new(3);
        assert!(steps.is_first());
        assert!(!steps.previous());
        assert!(steps.next());
        assert!(steps.next());
        assert!(steps.is_last());
        assert!(!steps.next());
        assert_eq!(steps.current(), 2);
        assert!(steps.previous());
        assert_eq!(steps.current(), 1);
    }

    #[test]
    fn jumps_only_to_existing_steps() {
        let mut steps = Steps::new(5);
        assert!(steps.go_to(4));
        assert!(!steps.go_to(5));
        assert_eq!(steps.current(), 4);
    }
}
