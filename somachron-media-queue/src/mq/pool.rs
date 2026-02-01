use std::sync::{mpsc, Arc, Mutex};

type Job<T> = Box<dyn FnOnce() -> T + Send + 'static>;

enum Message<T> {
    Job(Job<T>, mpsc::Sender<T>),
    Terminate,
}

struct Worker {
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    fn new<T: Send + 'static>(recv: Arc<Mutex<mpsc::Receiver<Message<T>>>>) -> Self {
        let thread = std::thread::spawn(move || loop {
            let message = recv.lock().unwrap().recv().unwrap();

            match message {
                Message::Job(fn_once, result_sender) => {
                    let result = fn_once();
                    let _ = result_sender.send(result);
                }
                Message::Terminate => break,
            };
        });

        Self {
            thread: Some(thread),
        }
    }
}

pub struct ThreadPool<T> {
    workers: Vec<Worker>,
    sender: mpsc::Sender<Message<T>>,
}

impl<T> ThreadPool<T>
where
    T: Send + 'static,
{
    pub fn new(size: usize) -> Self {
        let (sender, recv) = mpsc::channel();
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

    pub fn execute<F>(&self, f: F) -> mpsc::Receiver<T>
    where
        F: FnOnce() -> T + Send + 'static,
    {
        let (result_sender, result_recv) = mpsc::channel();

        self.sender.send(Message::Job(Box::new(f), result_sender)).unwrap();

        result_recv
    }
}

impl<T> Drop for ThreadPool<T> {
    fn drop(&mut self) {
        for _ in self.workers.iter() {
            self.sender.send(Message::Terminate).unwrap();
        }

        for worker in self.workers.iter_mut() {
            if let Some(handle) = worker.thread.take() {
                handle.join().unwrap();
            }
        }
    }
}
