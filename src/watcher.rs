use anyhow::Result;
use crossbeam_channel::{unbounded, Sender};
use notify::{
	Config, Error, PollWatcher, RecommendedWatcher, RecursiveMode,
	Watcher,
};
use notify_debouncer_mini::{
	new_debouncer, new_debouncer_opt, DebouncedEvent,
};
use scopetime::scope_time;
use std::{path::Path, thread, time::Duration};

pub struct RepoWatcher {
	receiver: crossbeam_channel::Receiver<()>,
}

impl RepoWatcher {
	pub fn new(workdir: &str, poll: bool) -> Self {
		log::trace!(
			"poll watcher: {poll} recommended: {:?}",
			RecommendedWatcher::kind()
		);

		let (tx, rx) = std::sync::mpsc::channel();

		let workdir = workdir.to_string();

		thread::spawn(move || {
			let timeout = Duration::from_secs(2);
			create_watcher(poll, timeout, tx, &workdir);
		});

		let (out_tx, out_rx) = unbounded();

		thread::spawn(move || {
			if let Err(e) = Self::forwarder(&rx, &out_tx) {
				//maybe we need to restart the forwarder now?
				log::error!("notify receive error: {}", e);
			}
		});

		Self { receiver: out_rx }
	}

	///
	pub fn receiver(&self) -> crossbeam_channel::Receiver<()> {
		self.receiver.clone()
	}

	fn forwarder(
		receiver: &std::sync::mpsc::Receiver<
			Result<Vec<DebouncedEvent>, Vec<Error>>,
		>,
		sender: &Sender<()>,
	) -> Result<()> {
		loop {
			let ev = receiver.recv()?;

			if let Ok(ev) = ev {
				log::debug!("notify events: {}", ev.len());

				for (idx, ev) in ev.iter().enumerate() {
					log::debug!("notify [{}]: {:?}", idx, ev);
				}

				if !ev.is_empty() {
					sender.send(())?;
				}
			}
		}
	}
}

fn create_watcher(
	poll: bool,
	timeout: Duration,
	tx: std::sync::mpsc::Sender<
		Result<Vec<DebouncedEvent>, Vec<Error>>,
	>,
	workdir: &str,
) {
	scope_time!("create_watcher");

	if poll {
		let config = Config::default()
			.with_poll_interval(Duration::from_secs(2));
		let mut bouncer = new_debouncer_opt::<_, PollWatcher>(
			timeout, None, tx, config,
		)
		.expect("Watch create error");
		bouncer
			.watcher()
			.watch(Path::new(&workdir), RecursiveMode::Recursive)
			.expect("Watch error");

		std::mem::forget(bouncer);
	} else {
		let mut bouncer = new_debouncer(timeout, None, tx)
			.expect("Watch create error");
		bouncer
			.watcher()
			.watch(Path::new(&workdir), RecursiveMode::Recursive)
			.expect("Watch error");

		std::mem::forget(bouncer);
	};
}
