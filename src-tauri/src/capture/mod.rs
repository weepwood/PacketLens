mod npcap;
mod service;

pub use npcap::{list_interfaces, npcap_available};
pub use service::CaptureService;
