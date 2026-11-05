mod c_family;

pub use self::c_family::{COMMENT_QUERY, TEST_GLOBS, TEST_QUERY};

#[cfg(feature = "c")]
pub mod c;

#[cfg(feature = "cpp")]
pub mod cpp;
