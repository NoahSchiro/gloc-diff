//! Churn gate thresholds, provided by the CLI and `.gloc.toml`.

/// How much churn is allowed before gloc fails, per bucket.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_src: Option<u64>,
    pub max_test: Option<u64>,
    pub max_total: Option<u64>,
}

impl Limits {
    /// Combine CLI and config-file limits per limit.
    pub fn merge(cli: Limits, file: Limits) -> Limits {
        Limits {
            max_src: cli.max_src.or(file.max_src),
            max_test: cli.max_test.or(file.max_test),
            max_total: cli.max_total.or(file.max_total),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_limits_win_over_file_limits() {
        let cli = Limits {
            max_src: Some(1),
            max_total: Some(2),
            ..Default::default()
        };
        let file = Limits {
            max_src: Some(3),
            max_test: Some(4),
            max_total: Some(5),
        };
        let merged = Limits::merge(cli, file);
        assert_eq!(merged.max_src, Some(1));
        assert_eq!(merged.max_test, Some(4));
        assert_eq!(merged.max_total, Some(2));
    }

    #[test]
    fn silent_cli_leaves_the_file_limits_in_place() {
        let file = Limits {
            max_test: Some(7),
            ..Default::default()
        };
        assert_eq!(Limits::merge(Limits::default(), file), file);
    }
}
