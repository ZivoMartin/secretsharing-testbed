use crate::configuration::Configuration;
use global_lib::{
    config_treatment::result_fields::ResultDuration, enc, messages::NodeCommand,
    process::ProcessTrait, task_pool::task::TaskInterface, OpId, Step,
};
use std::time::Instant;
use tokio::sync::mpsc::{channel, Receiver, Sender};
pub type InterfacePoolOutput = ResultDuration;

pub struct Process {
    receiver: Receiver<InterfacePoolOutput>,
    sender: TaskInterface<InterfacePoolOutput>,
    config: Configuration,
}

impl ProcessTrait<Configuration, InterfacePoolOutput, InterfacePoolOutput> for Process {
    /// This function takes in parameter a configuration and then initate the asked operation with the good number of node. This function assumes that the network has enough node to support the operation.
    /// The function returns a sender, the sender is used to tell to the process for a new output. At the end of the operation the process send the result via the given sender
    fn begin(
        config: Configuration,
        self_sender: TaskInterface<InterfacePoolOutput>,
    ) -> Sender<InterfacePoolOutput> {
        let (result_sender, self_receiver) = channel(100);
        tokio::spawn(async move {
            Process::start(config, self_sender, self_receiver).await;
        });
        result_sender
    }
}

impl Process {
    async fn start(
        config: Configuration,
        sender: TaskInterface<InterfacePoolOutput>,
        receiver: Receiver<InterfacePoolOutput>,
    ) {
        let mut process = Process {
            config,
            sender,
            receiver,
        };
        process.process().await;
    }

    async fn process(&mut self) {
        let fields = self.config.fields();
        println!("{:?} with {fields:?}", fields.step());
        let msg = enc!(Heart, NodeCommand::Process, fields);
        let id = self.id();
        let n = self.config.fields().n() as usize;
        self.config.network_mut().broadcast(msg, id, Some(n)).await;
        let result = self.wait_for_outputs().await;
        self.send_result(result).await;
    }

    pub fn id(&self) -> OpId {
        *self.config.id()
    }

    pub async fn send_result(&self, res: InterfacePoolOutput) {
        self.sender
            .output(res)
            .await
            .expect("Failed to send output message");
    }

    async fn wait_for_outputs(&mut self) -> InterfacePoolOutput {
        let n = self.config.fields().n();
        let t = self.config.fields().t();
        let timer = Instant::now();
        let mut final_result = 0;
        match self.config.fields().step() {
            Step::Sharing => {
                for _ in 0..n {
                    self.receiver.recv().await.expect("Failed to recv");
                }
                final_result = timer.elapsed().as_millis() as u64
            }
            Step::Reconstruct => {
                for i in 0..n {
                    self.receiver.recv().await.expect("Failed to recv");
                    if i == t + 1 {
                        final_result = timer.elapsed().as_millis() as u64;
                    }
                }
            }
        }
        final_result
    }
}
