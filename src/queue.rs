use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct WorkQueue<T> {
    q: Arc<Mutex<VecDeque<T>>>,
}

impl<T> WorkQueue<T> {
    pub async fn push(&mut self, x: T) {
        let mut q = self.q.lock().await;    
        q.push_back(x);
    }

    pub async fn len(&self) -> usize {
        let q = self.q.lock().await;
        q.len()
    }

    pub async fn find_first<F>(&mut self, f: F) -> Option<T>
        where F: Fn(&T) -> bool
    {
        let mut q = self.q.lock().await;
        let n = q.len();
        for _ in 0..n {
            let item = q.pop_front().unwrap();
            if f(&item) {
                return Some(item)
            } else {
                q.push_back(item);
            }
        }
        None
    }

    pub async fn pop(&mut self) -> Option<T> {
        let mut q = self.q.lock().await;
        q.pop_front()
    }
    
    pub async fn replace(&mut self, q: VecDeque<T>) {
        let mut val = self.q.lock().await;
        *val = q
    }

    pub fn from(queue: VecDeque<T>) -> WorkQueue<T> {
        WorkQueue {
            q: Arc::new(Mutex::new(queue))
        }
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
