use super::Receiver;
use crate::OpId;
use thiserror::Error;
use tracing::error;

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("Failed to send message to task {0}")]
    SendError(OpId),

    #[error("Failed to receive message")]
    ReceiveError,

    #[error("Task {0} is already closed")]
    TaskClosed(OpId),

    #[error("Task {0} creation failed")]
    TaskCreationError(OpId),

    #[error("Cleaning notifier error")]
    CleaningNotifierError,

    #[error("Task {0} already exists and is running")]
    TaskAlreadyExists(OpId),

    #[error("Task {0} creation sending failed")]
    TaskCreationSendingError(OpId),

    #[error("Failed to wait creation of task {0}")]
    FailedToWaitCreation(OpId),

    #[error("Task {0} doesn't exist")]
    TaskNotExist(OpId),

    #[error("Task {0} failed to give its output to the pool")]
    FailedToOutput(OpId),

    #[error("Failed to wait for all processes to complete, remaining: {0}")]
    CleaningTimer(usize),
}

pub type ErrorReceiver = Receiver<PoolError>;
pub type PoolResult<T> = Result<T, PoolError>;

/// This function is a default error handler, simply panic if receiving a message
pub fn panic_error_handler(mut error_receiver: ErrorReceiver) {
    tokio::spawn(async move {
        if let Some(e) = error_receiver.recv().await {
            panic!("{e}")
        }
    });
}

pub fn log_error_handler(mut error_receiver: ErrorReceiver) {
    tokio::spawn(async move {
        while let Some(e) = error_receiver.recv().await {
            error!("Task pool error: {:?}", e);
        }
    });
}

/// This function is a default error handler, simply ignoring all errors
pub fn ignore_error_handler(mut error_receiver: ErrorReceiver) {
    tokio::spawn(async move { while error_receiver.recv().await.is_some() {} });
}
