use std::{
    env,
    io::Read,
    process::exit,
    time::{Duration, Instant},
};
mod base_generator;
mod configuration;
mod network;
mod process;
use configuration::Configuration;
use global_lib::{
    config_treatment::{
        args::Args,
        fields::Fields,
        result_fields::{DebitCurves, ResultDuration},
    },
    dec, explicit_log, get_next_message,
    ip_addr::IpV4,
    log,
    messages::InterfaceCode,
    process_pool::ProcessPool,
    select,
    settings::{INTERFACE_PORT, LOCAL, LOCAL_IP, MANAGER_PORT, TIMEOUT, VERBOSE, WARM_UP},
    wrap, Evaluation, OpId, Step, Wrapped,
};
use network::Network;
use process::Process;
use std::fs::File;
use std::io::Write;
use tokio::{
    net::TcpListener,
    spawn,
    sync::mpsc::{channel, Receiver, Sender},
    time::sleep,
};
#[derive(Clone)]
pub struct Interface {
    log: Wrapped<File>,
    network: Network,
    process_pool: ProcessPool<ResultDuration, ResultDuration>,
    args: Wrapped<Args>,
    op_id: Wrapped<OpId>,
    cleaning_pool_sender: Wrapped<Option<Sender<u16>>>,
}

impl Interface {
    pub async fn new() -> (Interface, TcpListener) {
        let interface_ip = format!(
            "{}:{INTERFACE_PORT}",
            if LOCAL {
                LOCAL_IP.to_string()
            } else {
                local_ip_address::local_ip()
                    .expect("Failed to catch ip")
                    .to_string()
            }
        );
        let listener = TcpListener::bind(&interface_ip)
            .await
            .expect("Failed to bind interface");
        let interface = Interface {
            log: wrap!(File::create("../logs/interface").unwrap()),
            network: Network::default(),
            process_pool: ProcessPool::default(),
            args: wrap!(Args::default()),
            op_id: wrap!(0),
            cleaning_pool_sender: wrap!(None),
        };
        log!(interface, "Initializing interface on {interface_ip}");
        (interface, listener)
    }

    async fn new_command(self, ip: String, bytes: Vec<u8>, id: u64) {
        log!(self, "New command: {:?}", InterfaceCode::from(bytes[0]));
        select!(
            self_select, InterfaceCode, bytes, self,
            Connect => add_node ip,
            Output => new_output id,
            NodeReady => new_ready,
            PoolCleaned => new_pool_cleaned
        );
    }

    async fn new_pool_cleaned(&self, bytes: &[u8]) {
        let i = dec!(bytes);
        self.cleaning_pool_sender
            .lock()
            .await
            .as_ref()
            .expect("Pool is in cleaning phase but cleaning pool sender is none")
            .send(i)
            .await
            .unwrap();
    }

    async fn new_ready(&self, _bytes: &[u8]) {
        self.network.new_ready().await;
    }

    async fn op_id(&self) -> OpId {
        *self.op_id.lock().await
    }

    async fn inc_op_id(&self) {
        let mut id = self.op_id.lock().await;
        *id += 1;
    }

    fn handle_args(mut self) {
        let mut args = env::args();
        args.next().unwrap();
        let (path, managers_ip) = match args.next() {
            Some(f) => match &f as &str {
                "--regenerate" => {
                    if let Some(p) = args.next() {
                        Args::regenerate(p).unwrap()
                    } else {
                        println!("Please provide a path")
                    }
                    exit(0)
                }
                "--regenerate-from-details" => {
                    if let Some(p) = args.next() {
                        Args::regenerate_from_details(p).unwrap()
                    } else {
                        println!("Please provide a path")
                    }
                    exit(0)
                }
                "--merge" => {
                    if let Some(p1) = args.next() {
                        if let Some(p2) = args.next() {
                            if let Some(output) = args.next() {
                                Args::merge(p1, p2, output).unwrap()
                            } else {
                                println!("Please provide an output path")
                            }
                        } else {
                            println!("Please provide a second path")
                        }
                    } else {
                        println!("Please provide a first path")
                    }
                    exit(0)
                }
                _ => (
                    f,
                    get_managers(args.next().unwrap_or_else(|| {
                        panic!("Path to managers ips expected as a second argument")
                    })),
                ),
            },
            None => {
                eprintln!("You forgot to give a file to process !");
                exit(1);
            }
        };
        println!("Loading {path}");
        println!("Running with: VERBOSE: {VERBOSE}, LOCAL: {LOCAL}");
        spawn(async move {
            let args = Args::from_file(path);
            let n = args.get_maximum_network_size();
            log!(self, "Network of size {n}");
            self.args = wrap!(args);
            self.network.init_network(n, &managers_ip).await;
            if WARM_UP {
                self.warm_up().await;
            }
            self.clone().process_config().await;
        });
    }

    async fn warm_up(&mut self) {
        explicit_log!(self, "Warming up..");
        while !self.args.lock().await.warm_up_is_over() {
            self.start_operation_and_wait().await;
            // self.clean_the_pools(1, None).await;
            self.args.lock().await.warm_up_evolve();
        }
    }

    async fn process_config(mut self) {
        loop {
            let eval = self.args.lock().await.eval();
            self.setup_operations(&eval).await;
            println!("{eval:?}");
            match eval {
                Evaluation::Debit(_) => self.debit_evaluation().await,
                Evaluation::Latency(_) => self.latency_evaluation().await,
            };
            if self.args.lock().await.is_over() {
                break;
            }
        }
    }

    async fn setup_operations(&mut self, eval: &Evaluation) {
        if eval.is_reconstruct() {
            let mut f = self.args.lock().await.get_fields().unwrap().clone();
            f.set_step(Step::Sharing);
            self.start_operation_and_wait_with_fields(f).await;
        }
    }

    async fn latency_evaluation(&mut self) {
        explicit_log!(self, "Evaluating latency");
        self.network.switch_on_latency().await;
        let hmt = self.args.lock().await.hmt().unwrap();
        self.start_operation_and_wait().await; // To warm up
        for i in 0..hmt {
            explicit_log!(self, "Begining of the operation with hmt={}", i);
            let result = self.start_operation_and_wait().await;
            // self.clean_the_pools(1, None).await;
            self.args.lock().await.latency_evolve(result).unwrap();
        }
    }

    fn operation_spamer(
        mut self,
        mut end_receiver: Receiver<()>,
        latency: Duration,
        sender: Sender<usize>,
    ) {
        tokio::spawn(async move {
            let mut started = 0;
            while end_receiver.try_recv().is_err() && !self.all_args_consumed().await {
                self.start_operation().await;
                started += 1;
                log!(self, "Started: {started}");
                sleep(latency).await;
            }
            sender.send(started).await
        });
    }

    async fn debit_evaluation(&mut self) {
        explicit_log!(self, "Evaluating latency");
        let duration = Duration::from_secs(self.args.lock().await.debit_duration().unwrap() as u64);
        let b = self.args.lock().await.get_fields().unwrap().batch_size();
        let f = self.args.lock().await.get_fields().unwrap().clone();
        let mut base_latency = None;
        let hmt = 3;
        for _ in 0..hmt {
            let timer = Instant::now();
            self.start_operation_and_wait_with_fields(f.clone()).await;
            let l = timer.elapsed().as_millis();
            if base_latency.is_none() || base_latency.unwrap() < l {
                base_latency = Some(l)
            }
        }
        let average_latency = base_latency.unwrap();
        let base_latency = Duration::from_millis((average_latency as f32 * 1.05) as u64);
        explicit_log!(self, "Base average latency: {average_latency}");

        let mut curve = DebitCurves::new();
        let mut current_latency = base_latency;
        let mut i = 1;
        let mut stop = false;
        let mut prev = None;

        while !stop {
            log!(self, "Begin with i = {i}");
            let (end_sender, end_receiver) = channel(100);
            let (nb_started_sender, mut nb_started_receiver) = channel(100);
            self.clone()
                .operation_spamer(end_receiver, current_latency, nb_started_sender);

            let mut receiver = self.process_pool.new_result_redirection().await;
            let mut counter = 0;
            let mut latency_sum = 0;
            let timer = Instant::now();
            while timer.elapsed() < duration {
                let output = receiver.recv().await.unwrap();
                counter += 1;
                latency_sum += *output.output;
                explicit_log!(self, "{counter}");
            }
            latency_sum /= counter;
            explicit_log!(self, "SENDING END FLAG");
            end_sender.send(()).await.unwrap();
            receiver.close();

            log!(self, "Waiting for spamer to send its message");
            let to_wait_for = nb_started_receiver.recv().await.unwrap();
            explicit_log!(self, "Waiting for emptying the pool");

            self.clean_the_pools(to_wait_for - counter as usize, Some(TIMEOUT))
                .await;

            if prev.is_some() && prev.unwrap() > counter {
                stop = true
            }

            prev = Some(counter);

            counter /= duration.as_secs();
            counter *= b as u64;

            if curve.is_empty() || curve.last() != counter {
                curve.push(counter, latency_sum);
            }
            explicit_log!(self, "old latency {current_latency:?}");
            let latency = (base_latency.as_millis() as f32 * (1.0 - (i as f32 / 13.0))) as u64;
            current_latency = Duration::from_millis(if latency > 0 {
                latency
            } else {
                current_latency.as_millis() as u64 - 1
            });
            i += 1;
            explicit_log!(self, "New latency: {current_latency:?}");
        }

        let _ = self.args.lock().await.debit_evolve(curve);
        println!("Process is over");
    }

    async fn process_operation_with_fields(&mut self, fields: Fields) {
        self.inc_op_id().await;
        log!(self, "New op ID: {}", self.op_id().await);
        let config = Configuration::new(fields, self.op_id().await, self.network.clone());
        self.process_pool
            .new_task::<Process, Configuration>(self.op_id().await, config)
            .await
            .unwrap();
    }

    async fn start_operation(&mut self) {
        let f = self.args.lock().await.get_fields().unwrap().clone();
        log!(self, "Starting an operation with fields {f:?}");
        self.process_operation_with_fields(f).await
    }

    pub async fn start_operation_and_wait_with_fields(&mut self, fields: Fields) -> ResultDuration {
        let mut receiver = self.process_pool.new_result_redirection().await;
        self.process_operation_with_fields(fields).await;
        *receiver.recv().await.expect("Failed to recv result").output
    }

    async fn start_operation_and_wait(&mut self) -> ResultDuration {
        let f = match self.args.lock().await.get_fields() {
            Some(fields) => fields.clone(),
            _ => panic!("Config is over"),
        };
        log!(self, "Starting and waiting an operation with fields {f:?}");
        let res = self.start_operation_and_wait_with_fields(f).await;
        log!(self, "Operation succeed {res}");
        res
    }

    async fn new_output(&self, bytes: &[u8], id: OpId) {
        log!(self, "New output on {id}");
        let result: ResultDuration = dec!(bytes);
        self.process_pool.send(id, result).await.unwrap()
    }

    /// Add a node in the network. Bytes contains the port of the node.
    async fn add_node(&self, bytes: &[u8], ip: String) {
        let port: u16 = dec!(bytes, u16);
        let ip = IpV4::new(IpV4::extract_ip_from_str(&ip).expect("Invalid ip"), port);
        self.network.add_node(ip).await;
    }

    async fn clean_the_pools(&self, _started: usize, _dur: Option<tokio::time::Duration>) {
        tokio::time::sleep(tokio::time::Duration::from_millis(30_000)).await; // Waiting 30 seconds
        return;
    }

    async fn all_args_consumed(&self) -> bool {
        self.args.lock().await.is_over()
    }
}

fn get_managers(path: String) -> Vec<IpV4> {
    let mut f = File::open(&path).unwrap_or_else(|e| panic!("Failed to open file {path}: {e}"));
    let mut s = String::new();
    f.read_to_string(&mut s)
        .unwrap_or_else(|e| panic!("Failed to read file {path}: {e}"));
    s.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|s| {
            IpV4::new(
                IpV4::ip_from_str(&s).expect("Machines ip file invalid"),
                MANAGER_PORT,
            )
        })
        .collect::<Vec<IpV4>>()
}

async fn listen_on_interface(interface: Interface, listener: TcpListener) {
    loop {
        let (mut socket, ip) = listener.accept().await.unwrap();
        let interface = interface.clone();
        spawn(async move {
            loop {
                let (message_buf, _, id) = match get_next_message(&mut socket).await {
                    Some(b) => b,
                    _ => return,
                };
                interface
                    .clone()
                    .new_command(ip.to_string(), message_buf, id)
                    .await;
            }
        });
    }
}

#[tokio::main]
async fn main() {
    let (interface, listener) = Interface::new().await;
    interface.clone().handle_args();
    listen_on_interface(interface, listener).await;
}
