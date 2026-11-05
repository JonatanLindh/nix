//! The tokio runtime the services run on.
//!
//! gpui has its own executor, but bluer, nmrs, zbus and the rest want tokio. Services run here
//! and talk to the UI through futures channels and join handles, which any executor can await:
//! a gpui task awaits a [`spawn`]ed future or reads a [`stream`] without knowing about tokio.

use std::sync::LazyLock;

use futures::channel::mpsc;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("services")
        .enable_all()
        .build()
        .expect("tokio runtime")
});

/// Runs `future` on the services runtime. The handle resolves to its output from any executor.
pub fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    RUNTIME.spawn(future)
}

/// Runs `future` on the services runtime and forgets it.
pub fn detach<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    drop(RUNTIME.spawn(future));
}

/// A stream of what `produce` sends, run on the services runtime with a channel of `capacity`.
/// `produce` stops (its sends fail) once the receiver is dropped.
pub fn stream<T, Fut>(
    capacity: usize,
    produce: impl FnOnce(mpsc::Sender<T>) -> Fut,
) -> mpsc::Receiver<T>
where
    T: Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = mpsc::channel(capacity);
    detach(produce(tx));
    rx
}

/// Runs `future` on the services runtime, then hands its output to `apply` on the entity behind
/// `cx`, back on the UI thread. Nothing happens if the entity is gone by then.
pub fn then<E, R>(
    cx: &mut gpui::Context<E>,
    future: impl Future<Output = R> + Send + 'static,
    apply: impl FnOnce(&mut E, R, &mut gpui::Context<E>) + 'static,
) where
    E: 'static,
    R: Send + 'static,
{
    let task = spawn(future);
    cx.spawn(async move |this, cx| {
        if let Ok(output) = task.await {
            let _ = this.update(cx, |entity, cx| apply(entity, output, cx));
        }
    })
    .detach();
}
