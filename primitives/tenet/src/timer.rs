use std::time::Duration;
use tokio::time::interval;

use crate::model::PoM;

async fn call_timeout_timer(interval_seconds: u64, pom: PoM) {
	let mut interval = interval(Duration::from_secs(interval_seconds));
	interval.tick().await;
	crate::call_tree::check_start_challenge(pom);
}

pub fn start_call_timer(pom: PoM) {
	tokio::spawn(call_timeout_timer(pom.timeout, pom));
}
