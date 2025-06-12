use std::time::Duration;
pub static VERBOSE: bool = true;
pub static LOCAL: bool = false;
pub static WARM_UP: bool = !LOCAL;
pub static WITH_TITLE: bool = false;
pub static DEBIT_CURVE_SLEEP_DURATION: Duration = Duration::from_millis(1000);
pub static DEBIT_CURVE_NB_POINT: usize = 10;
pub static LATENCY_LIMIT: u128 = 10;
pub static SPAMER_SLEEP_DURATION: Duration = Duration::from_millis(10);
pub static SPAMER_LATENCY_LIMIT: u128 = 1;

pub static LOCAL_IP: &str = "127.0.0.1";
pub static SPAMER_REDUCTION: f32 = 0.8;
pub static TIMEOUT: Duration = Duration::from_secs(25);

pub const BASE_CAPACITY: usize = 2000;
pub static INTERFACE_PORT: u16 = 18_800;
pub static MANAGER_PORT: u16 = 17_000;
