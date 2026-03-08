use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

enum Message<T> {
    Job(Job<T>, mpsc::UnboundedSender<T>),
    Terminate,
}

struct Worker {
    handle: Option<tokio::task::JoinHandle<()>>,
}

impl Worker {
    fn new<T: Send + 'static>(recv: Arc<Mutex<mpsc::UnboundedReceiver<Message<T>>>>) -> Self {
        let handle = tokio::spawn(async move {
            loop {
                let message = recv.lock().await.recv().await;

                if let Some(message) = message {
                    match message {
                        Message::Job(fn_once, result_sender) => {
                            let result = tokio::task::spawn_blocking(move || fn_once()).await.unwrap();
                            let _ = result_sender.send(result);
                        }
                        Message::Terminate => break,
                    }
                }
            }
        });

        Self {
            handle: Some(handle),
        }
    }
}

pub struct ThreadPool<T> {
    workers: Vec<Worker>,
    sender: mpsc::UnboundedSender<Message<T>>,
}

impl<T> ThreadPool<T>
where
    T: Send + 'static,
{
    pub fn new(size: usize) -> Self {
        let (sender, recv) = mpsc::unbounded_channel();
        let recv = Arc::new(Mutex::new(recv));

        let mut workers = Vec::with_capacity(size);
        for _ in 0..size {
            workers.push(Worker::new(Arc::clone(&recv)));
        }

        Self {
            workers,
            sender,
        }
    }

    pub fn execute<F>(&self, f: F) -> mpsc::UnboundedReceiver<T>
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let (result_sender, result_recv) = mpsc::unbounded_channel();

        self.sender.send(Message::Job(Box::new(f), result_sender)).unwrap();

        result_recv
    }
}

impl<T> Drop for ThreadPool<T> {
    fn drop(&mut self) {
        for _ in self.workers.iter() {
            let _ = self.sender.send(Message::Terminate);
        }

        // Note: In async context, you'd typically want to await these
        // But Drop is not async, so we just drop the handles
        for worker in self.workers.iter_mut() {
            if let Some(handle) = worker.handle.take() {
                handle.abort();
            }
        }
    }
}
