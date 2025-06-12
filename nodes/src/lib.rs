pub mod avss_simpl;
pub mod badger;
// pub mod beacon;
pub mod bingo;
pub mod broadcast;
pub mod crypto;
pub mod disperse_retrieve;
pub mod hbavss;
pub mod lightweight;
pub mod macros;
pub mod node;
pub mod one_sided_vote;
// pub mod proc_macro;
pub mod haven;
pub mod secure_message_dist;
pub mod system;

use global_lib::OpId;

#[allow(dead_code)]
fn node_log_file(_id: OpId, _index: u16) -> String {
    String::from("log_nodes")
}
