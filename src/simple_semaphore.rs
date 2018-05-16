// This is a simple counting semaphore implementaion using a mutex and a condvar
// It was implemented because std::sync::Semaphore is deprecated since Rust v1.70

use std::sync::{Mutex, Condvar};

#[derive(Debug, Default)]
pub struct Semaphore {
    mutex: Mutex<isize>,
    condvar: Condvar,
}

impl Semaphore {
    pub fn new(init_val: isize) -> Semaphore {
        Semaphore {
            mutex: Mutex::new(init_val),
            condvar: Condvar::new(),
        }
    }

    pub fn wait(&self) {
        let mut sem_count = self.mutex.lock().unwrap();
        while *sem_count <= 0 {
            sem_count = self.condvar.wait(sem_count).unwrap();
        }
        *sem_count -= 1;
    }

    pub fn signal(&self){
        *self.mutex.lock().unwrap() += 1;
        self.condvar.notify_one();
    }
}
