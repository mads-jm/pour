/// Re-export so all existing `crate::util::atomic_replace` and
/// `pour::util::atomic_replace` call-sites continue to compile without change.
///
/// The canonical implementation lives at [`crate::transport::atomic::atomic_replace`].
pub use crate::transport::atomic::{atomic_replace, resolve_write_target};
