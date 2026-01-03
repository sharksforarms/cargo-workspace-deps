use std::fmt;

#[derive(Debug)]
pub enum CheckFailure {
    Consolidation(usize),
    Conflicts(usize),
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckFailure::Consolidation(count) => {
                write!(
                    f,
                    "Check failed: {} dependencies could be consolidated",
                    count
                )
            }
            CheckFailure::Conflicts(count) => {
                write!(f, "Check failed: {} unresolved conflicts", count)
            }
        }
    }
}

impl std::error::Error for CheckFailure {}
