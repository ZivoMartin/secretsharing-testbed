pub use super::task_pool::task_pool::PoolTaskEnded as PoolProcessEnded;

pub use super::task_pool::{errors::PoolError, TaskPool as ProcessPool, TaskTrait as ProcessTrait};
