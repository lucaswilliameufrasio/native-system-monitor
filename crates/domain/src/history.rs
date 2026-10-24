use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub struct MetricHistory {
    capacity: usize,
    values: VecDeque<f32>,
}

impl MetricHistory {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            values: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, value: f32) {
        if self.capacity == 0 {
            return;
        }

        if self.values.len() == self.capacity {
            let _oldest = self.values.pop_front();
        }

        self.values.push_back(value);
    }

    pub fn values(&self) -> Vec<f32> {
        self.values.iter().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::MetricHistory;

    #[test]
    fn history_retains_only_the_newest_values() {
        let mut history = MetricHistory::new(3);

        for value in [1.0, 2.0, 3.0, 4.0] {
            history.push(value);
        }

        assert_eq!(history.values(), vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn zero_capacity_history_discards_values() {
        let mut history = MetricHistory::new(0);

        history.push(1.0);

        assert!(history.values().is_empty());
    }
}
