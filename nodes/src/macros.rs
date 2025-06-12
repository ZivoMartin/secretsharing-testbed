#[macro_export]
macro_rules! create_channels {
    ($node:ident, $($function:ident),*) => {
        $(
            let node_cloned = $node.clone();
            $node.lock().await.push_handler(tokio::spawn(async move { $function(node_cloned).await }));
        )*
    }
}

#[macro_export]
macro_rules! panic_if_over {
    ($receiver:expr) => {
        if let Some(msg) = $receiver.recv().await {
            if $crate::system::message_interface::SendableMessage::is_close(&msg) {
                $receiver.close();
                panic!("Channel is closed.")
            }
            msg
        } else {
            $receiver.close();
            panic!("Failed to receiv a message: all senders droped.")
        }
    };
}

#[macro_export]
macro_rules! break_if_over {
    ($receiver:expr) => {
        if let Some(msg) = $receiver.recv().await {
            if $crate::system::message_interface::SendableMessage::is_close(&msg) {
                $receiver.close();
                break;
            }
            msg
        } else {
            $receiver.close();
            break;
        }
    };
}
