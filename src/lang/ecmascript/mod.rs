mod ecma;

pub use self::ecma::{COMMENT_QUERY, TEST_GLOBS, TEST_QUERY};

#[cfg(feature = "javascript")]
pub mod javascript;

#[cfg(feature = "typescript")]
pub mod typescript;
