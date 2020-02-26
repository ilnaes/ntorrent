use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct WorkQueue<T> {
    q: Arc<Mutex<VecDeque<T>>>,
}

impl<T> WorkQueue<T> {
    pub async fn push(&mut self) {
        // let mut q = self.q.lock().await;    
        // q.push_back(x);
    }

    pub async fn pop(&mut self) -> Option<T> {
        let mut q = self.q.lock().await;
        q.pop_front()
    }
    
    pub fn replace(&mut self, q: VecDeque<T>) {
        self.q = Arc::new(Mutex::new(q));
    }

    pub fn new() -> WorkQueue<T> {
        WorkQueue {
            q: Arc::new(Mutex::new(VecDeque::new()))
        }
    }

    pub fn clone(&self) -> WorkQueue<T> {
        WorkQueue {
            q: Arc::clone(&self.q),
        }
    }
}
