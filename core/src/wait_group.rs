use futures::task::AtomicWaker;
use std::{
	future::Future,
	pin::Pin,
	sync::{
		Arc,
		atomic::{AtomicUsize, Ordering},
	},
	task::{Context, Poll},
};

#[derive(Clone)]
pub struct WaitGroup {
	state: Arc<State>,
}

struct State {
	/// number of outstanding tasks
	counter: AtomicUsize,
	/// wakes the waiter when counter == 0
	waker: AtomicWaker,
}

impl WaitGroup {
	/// Create a new, empty wait group.
	pub fn new() -> Self {
		WaitGroup {
			state: Arc::new(State {
				counter: AtomicUsize::new(0),
				waker: AtomicWaker::new(),
			}),
		}
	}

	/// Increment the wait-group counter by `n`.
	pub fn add(&self, n: usize) {
		let prev = self.state.counter.fetch_add(n, Ordering::AcqRel);
		if prev == 0 {
			self.state.waker.take();
		}
	}

	/// Decrement the wait-group counter by 1, and wake if it hits zero.
	pub fn done(&self) {
		let prev = self.state.counter.fetch_sub(1, Ordering::AcqRel);
		assert!(prev != 0, "WaitGroup counter underflow");
		if prev == 1 {
			self.state.waker.wake();
		}
	}

	/// Returns a Future that completes when counter == 0.
	pub fn wait(&self) -> WaitFuture {
		WaitFuture {
			state: self.state.clone(),
		}
	}

	/// RAII-style registration: increments counter now,
	/// and will call `done()` when the guard is dropped.
	pub fn register(&self) -> WaitGroupGuard {
		self.add(1);
		WaitGroupGuard { wg: self.clone() }
	}
}

/// A guard which decrements the wait group when dropped.
pub struct WaitGroupGuard {
	wg: WaitGroup,
}

impl Drop for WaitGroupGuard {
	fn drop(&mut self) {
		self.wg.done();
	}
}

pub struct WaitFuture {
	state: Arc<State>,
}

impl Future for WaitFuture {
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
		if self.state.counter.load(Ordering::Acquire) == 0 {
			return Poll::Ready(());
		}

		self.state.waker.register(cx.waker());

		// Check again in case it hit zero between load() and register().
		if self.state.counter.load(Ordering::Acquire) == 0 {
			Poll::Ready(())
		} else {
			Poll::Pending
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use futures::executor::block_on;
	use std::{thread, time::Duration};

	#[test]
	fn wait_completes_when_zero() {
		let wg = WaitGroup::new();
		wg.add(1);
		wg.done();
		block_on(wg.wait());
	}

	#[test]
	fn wait_blocks_until_done() {
		let wg = WaitGroup::new();
		wg.add(1);
		let wg_clone = wg.clone();
		thread::spawn(move || {
			thread::sleep(Duration::from_millis(50));
			wg_clone.done();
		});
		block_on(wg.wait());
	}

	#[test]
	fn guard_decrements_on_drop() {
		let wg = WaitGroup::new();
		{
			let _guard = wg.register();
		}
		block_on(wg.wait());
	}

	#[test]
	fn multiple_tasks_complete() {
		let wg = WaitGroup::new();
		wg.add(2);
		let wg1 = wg.clone();
		let wg2 = wg.clone();
		thread::spawn(move || {
			thread::sleep(Duration::from_millis(10));
			wg1.done();
		});
		thread::spawn(move || {
			thread::sleep(Duration::from_millis(20));
			wg2.done();
		});
		block_on(wg.wait());
	}
}
