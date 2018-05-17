#![allow(dead_code)]

use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use simple_semaphore::{*};

const NTHREADS: u32 = 4;

#[derive(Clone, Debug)]
pub struct Stats {
    pub loops: u32,
    pub rules: u32,
    pub betas: u32,
    pub dupls: u32,
    pub annis: u32
}

#[derive(Clone, Debug)]
pub struct Net {
    pub nodes: Vec<u32>,
    pub reuse: Vec<u32>
}

pub struct Locks {
    pub warp: Mutex<Vec<u32>>,
    pub warp_queue: Semaphore,
    pub stats: Mutex<Stats>,
    pub active_threads: Mutex<u32>,
    pub net: RwLock<Net>,
    //pub reuse: RwLock<()>
}

impl Locks {
    pub fn new(net: Net) -> Locks {
        Locks {
            warp: Mutex::new(Vec::new()),
            warp_queue: Semaphore::new(0),
            stats: Mutex::new(Stats {
                loops: 0,
                rules: 0,
                betas: 0,
                dupls: 0,
                annis: 0
            }),
            active_threads: Mutex::new(0),
            net: RwLock::new(net),
            //reuse: RwLock::new(())
        }
    }
}

pub type Port = u32;

// TODO: Refactor this function to avoid code repetition
pub fn new_node(net : &mut Net, kind : u32, locks: Option<Arc<Locks>>) -> u32 {

    let reuse = match &locks {
        &Some(ref lock) => {
            // acquire NET_REUSE mutex
            let mut reuse_lock = lock.net.write().unwrap();
            //net.reuse.pop()
            reuse_lock.reuse.pop()
            // NET_REUSE mutex released
        },
        &None => net.reuse.pop()
    };


    let node : u32 = match reuse {
        Some(index) => index,
        None        => {
            match &locks {
                &Some(ref lock) => {
                    // acquire NET_EDIT mutex
                    let mut net_lock = lock.net.write().unwrap();
                    let len = net_lock.nodes.len();
                    net_lock.nodes.resize(len + 4, 0);
                    (len as u32) / 4
                    // NET_EDIT mutex released
                }
                &None => {
                    let len = net.nodes.len();
                    net.nodes.resize(len + 4, 0);
                    (len as u32) / 4
                },
            }
        }
    };

    match &locks {
        &Some(ref lock) => {
            // acquire NET_EDIT mutex
            let mut net_lock = lock.net.write().unwrap();
            net_lock.nodes[port(node, 0) as usize] = port(node, 0);
            net_lock.nodes[port(node, 1) as usize] = port(node, 1);
            net_lock.nodes[port(node, 2) as usize] = port(node, 2);
            net_lock.nodes[port(node, 3) as usize] = kind << 2;
            // NET_EDIT mutex released
        }
        &None => {
            net.nodes[port(node, 0) as usize] = port(node, 0);
            net.nodes[port(node, 1) as usize] = port(node, 1);
            net.nodes[port(node, 2) as usize] = port(node, 2);
            net.nodes[port(node, 3) as usize] = kind << 2;
        }
    }
    return node;
}

pub fn port(node : u32, slot : u32) -> Port {
    (node << 2) | slot
}

pub fn node(port : Port) -> u32 {
    port >> 2
}

pub fn slot(port : Port) -> u32 {
    port & 3
}

// !! UNSAFE !!
pub fn enter(net : &Net, port : Port) -> Port {
    net.nodes[port as usize]
}

// !! UNSAFE !!
pub fn kind(net : &Net, node : u32) -> u32 {
    net.nodes[port(node, 3) as usize] >> 2
}

// !! UNSAFE !!
pub fn meta(net : &Net, node : u32) -> u32 {
    net.nodes[port(node, 3) as usize] & 3
}

// !! UNSAFE !!
pub fn set_meta(net : &mut Net, node : u32, meta : u32) {
    let ptr = port(node, 3) as usize;
    net.nodes[ptr] = net.nodes[ptr] & 0xFFFFFFFC | meta;
}

// !! UNSAFE !!
pub fn link(net : &mut Net, ptr_a : u32, ptr_b : u32) {
    net.nodes[ptr_a as usize] = ptr_b;
    net.nodes[ptr_b as usize] = ptr_a;
}

// !! UNSAFE !!
pub fn reduce(_locks : Locks) -> (Stats, Net) {
    //let stats = Stats { loops: 0, rules: 0, betas: 0, dupls: 0, annis: 0 };
    //let mut warp : Vec<u32> = Vec::new();
    let mut locks = Arc::new(_locks);
    let net = locks.net.write().unwrap();
    let mut next : Port = net.nodes[0];
    drop(net);
    let mut prev : Port = 0;
    let mut back : Port = 0;
    let mut handles = vec![];

    // spawn threads
    for _ in 1..NTHREADS {
        let locks = Arc::clone(&locks);
        //let s_net = Arc::clone(&s_net);
        handles.push(thread::spawn (move || {
            thread_alg(locks);
        }));
    }

    while (next > 0) /*|| (warp.len() > 0)*/ {
        reduce_iteration(/*s_net,*/ &mut next, &mut prev, &mut back, &locks);
    }

    for handle in handles {
        let _ = handle.join();
    }

    let stats = locks.stats.lock().unwrap().clone();
    let net = locks.net.read().unwrap().clone();
    (stats, net)
    //stats
}

// !! UNSAFE !!
pub fn rewrite(net : &mut Net, x : Port, y : Port) {
    if kind(net, x) == kind(net, y) {
        let p0 = enter(net, port(x, 1));
        let p1 = enter(net, port(y, 1));
        link(net, p0, p1);
        let p0 = enter(net, port(x, 2));
        let p1 = enter(net, port(y, 2));
        link(net, p0, p1);
        net.reuse.push(x);
        net.reuse.push(y);
    } else {
        let t = kind(net, x);
        let a = new_node(net, t, None); // <-------- Should receive Some(lock)!
        let t = kind(net, y);
        let b = new_node(net, t, None); // <-------- Should receive Some(lock)!
        let t = enter(net, port(x, 1));
        link(net, port(b, 0), t);
        let t = enter(net, port(x, 2));
        link(net, port(y, 0), t);
        let t = enter(net, port(y, 1));
        link(net, port(a, 0), t);
        let t = enter(net, port(y, 2));
        link(net, port(x, 0), t);
        link(net, port(a, 1), port(b, 1));
        link(net, port(a, 2), port(y, 1));
        link(net, port(x, 1), port(b, 2));
        link(net, port(x, 2), port(y, 2));
        set_meta(net, x, 0);
        set_meta(net, y, 0);
    }
}

// !! UNSAFE !!
// thread_alg(net, &next, &prev, &back, &mut warp, locks);
fn thread_alg(/*net: &mut Net, */locks: Arc<Locks>) {
    let net = locks.net.read().unwrap();
    let mut next : Port = net.nodes[0];
    drop(net);
    let mut prev : Port = 0;
    let mut back : Port = 0;
    loop {
        // wait on WARP_QUEUE semaphore
        // fetch ACTIVE mutex
        // add ACTIVE counter
        // release ACTIVE mutex

        while (next > 0)/* || (warp.len() > 0)*/ {
            reduce_iteration(/*net, */&mut next, &mut prev, &mut back, &locks);
        }

        // fetch ACTIVE mutex
        // subtract ACTIVE counter
        // release ACTIVE mutex
    }

}


fn reduce_iteration(/*net: &mut Net, */next: &mut u32, prev: &mut u32, back: &mut u32, locks: &Arc<Locks>) {
        *next = if *next == 0 {
            let index = {
                //acquire WARP Mutex
                //warp.pop().unwrap()
                // WARP mutex released
                0
            };
            let net = locks.net.read().unwrap();
            enter(&net, port(index, 2))
        }
        else {
            *next
        };
        let net = locks.net.read().unwrap();
        *prev = enter(&net, *next);
        drop(net);
        if slot(*next) == 0 && slot(*prev) == 0 && node(*prev) != 0 {
            // acquire STATS mutex
            //stats.rules += 1;
            // STATS mutex released
            let mut net = locks.net.write().unwrap();
            *back = enter(&net, port(node(*prev), meta(&net, node(*prev))));
            rewrite(&mut net, node(*prev), node(*next));
            *next = enter(&net, *back);
        } else if slot(*next) == 0 {
            // aquire WARP mutex
            //warp.push(node(*next));
            // WARP mutex released
            // signal WARP_QUEUE semaphore
            let net = locks.net.read().unwrap();
            *next = enter(&net, port(node(*next), 1));
        } else {
            let mut net = locks.net.write().unwrap();
            set_meta(&mut net, node(*next), slot(*next));
            *next = enter(&net, port(node(*next), 0));
        }
        // acquire STATS mutex
        //stats.loops += 1;
        // STATS mutex released
}
