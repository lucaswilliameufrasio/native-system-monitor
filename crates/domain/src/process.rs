#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub start_time_unix_seconds: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProcessMetrics {
    pub identity: ProcessIdentity,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub cpu_usage_percent: Option<f32>,
    pub memory_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::ProcessIdentity;
    use std::collections::HashSet;

    #[test]
    fn process_start_time_distinguishes_reused_pids() {
        let original = ProcessIdentity {
            pid: 42,
            start_time_unix_seconds: 100,
        };
        let reused_pid = ProcessIdentity {
            pid: 42,
            start_time_unix_seconds: 200,
        };
        let identities = HashSet::from([original, reused_pid]);

        assert_eq!(identities.len(), 2);
    }
}
