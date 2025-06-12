pub mod errors;
pub mod task;
pub mod task_pool;

pub type Wrapped<T> = Arc<RwLock<T>>;

use std::sync::Arc;
pub use task::TaskTrait;
pub use task_pool::{OpId, TaskPool};
pub use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
