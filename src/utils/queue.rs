use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};

// thread safe queue
#[derive(Debug)]
pub struct Queue<T> {
    q: Arc<Mutex<VecDeque<T>>>,
    cond: Arc<Notify>,
}

impl<T> Queue<T> {
    pub async fn push(&mut self, x: T) {
        let mut q = self.q.lock().await;
        q.push_back(x);
        self.cond.notify();
    }

    pub fn get_q(&self) -> Arc<Mutex<VecDeque<T>>> {
        Arc::clone(&self.q)
    }

    pub async fn len(&self) -> usize {
        let q = self.q.lock().await;
        q.len()
    }

    // find first item that satisfies the f filter
    // does not block
    pub async fn find_first<F>(&mut self, f: F) -> Option<T>
    where
        F: Fn(&T) -> bool,
    {
        let mut q = self.q.lock().await;
        let n = q.len();
        for _ in 0..n {
            let item = q.pop_front().unwrap();
            if f(&item) {
                self.cond.notify();
                return Some(item);
            } else {
                q.push_back(item);
            }
        }
        self.cond.notify();
        None
    }

    // blocking pop
    pub async fn pop_block(&mut self) -> T {
        loop {
            let mut q = self.q.lock().await;
            let ret = q.pop_front();
            if let Some(res) = ret {
                return res;
            }

            // unlock and sleep
            drop(q);
            self.cond.notified().await;
        }
    }

    pub async fn clear(&mut self) {
        let mut q = self.q.lock().await;
        *q = VecDeque::new();
    }

    // nonblocking pop, can return None
    // pub async fn pop(&mut self) -> Option<T> {
    //     let mut q = self.q.lock().await;

    //     // notify on way out so don't start blockeds
    //     self.cond.notify();
    //     q.pop_front()
    // }

    pub async fn replace(&mut self, q: VecDeque<T>) {
        let mut val = self.q.lock().await;
        *val = q;
        self.cond.notify();
    }

    pub fn from(queue: VecDeque<T>) -> Queue<T> {
        Queue {
            q: Arc::new(Mutex::new(queue)),
            cond: Arc::new(Notify::new()),
        }
    }

    pub fn new() -> Queue<T> {
        Queue {
            q: Arc::new(Mutex::new(VecDeque::new())),
            cond: Arc::new(Notify::new()),
        }
    }

    pub fn clone(&self) -> Queue<T> {
        Queue {
            q: Arc::clone(&self.q),
            cond: Arc::clone(&self.cond),
        }
    }
}

#[cfg(test)]
mod test {
    use super::Queue;
    use std::time::Duration;
    use tokio::time::timeout;

    // #[tokio::test]
    // async fn test_push_pop() {
    // let mut q = Queue::new();
    // assert_eq!(q.pop().await, None);

    // q.push(1).await;
    // assert_eq!(q.pop().await, Some(1));
    // assert_eq!(q.pop().await, None);
    // }

    #[tokio::test]
    async fn test_replace() {
        let mut q = Queue::new();
        q.replace(vec![3, 2, 1].into_iter().collect()).await;

        assert_eq!(q.pop_block().await, 3);
        assert_eq!(q.pop_block().await, 2);
    }

    #[tokio::test]
    async fn test_block() {
        let mut q = Queue::<i64>::new();
        let res = timeout(Duration::from_millis(100), q.pop_block())
            .await
            .ok();
        assert_eq!(res, None);

        q.push(1).await;
        let res = timeout(Duration::from_millis(100), q.pop_block())
            .await
            .ok();
        assert_eq!(res, Some(1));

        let mut q1 = q.clone();
        tokio::spawn(async move {
            timeout(Duration::from_millis(50), q1.push(2)).await.ok();
        });
        let res = timeout(Duration::from_millis(100), q.pop_block())
            .await
            .ok();
        assert_eq!(res, Some(2));
    }
}
