#![allow(dead_code)]

use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use simple_semaphore::*;

const MULTITHREADING: bool = true;

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
    warp: Mutex<Vec<u32>>,
    warp_queue: Semaphore,
    stats: Mutex<Stats>,
    active_threads: Mutex<u32>,
    net: Mutex<()>,
    reuse: Mutex<()>
}

impl Locks {
    pub fn new() -> Locks {
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
            net: Mutex::new(()),
            reuse: Mutex::new(())
        }
    }
}

pub type Port = u32;

// TODO: Refactor this function to avoid code repetition
pub fn new_node(net : &mut Net, kind : u32, locks: Option<Arc<Locks>>) -> u32 {

    let reuse = match &locks {
        &Some(ref lock) => {
            // acquire NET_REUSE mutex
            let reuse_lock = lock.reuse.lock().unwrap();
            net.reuse.pop()
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
                    let net_lock = lock.net.lock().unwrap();
                    let len = net.nodes.len();
                    net.nodes.resize(len + 4, 0);
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
            let net_lock = lock.net.lock().unwrap();
            net.nodes[port(node, 0) as usize] = port(node, 0);
            net.nodes[port(node, 1) as usize] = port(node, 1);
            net.nodes[port(node, 2) as usize] = port(node, 2);
            net.nodes[port(node, 3) as usize] = kind << 2;
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
pub fn reduce(net : &mut Net) -> Stats {
    let mut stats = Stats { loops: 0, rules: 0, betas: 0, dupls: 0, annis: 0 };
    let mut warp : Vec<u32> = Vec::new();
    let mut next : Port = net.nodes[0];
    let mut prev : Port;
    let mut back : Port;
    let locks = Arc::new(Locks::new());

    while (next > 0) || (warp.len() > 0) {
        next = if next == 0 { enter(net, port(warp.pop().unwrap(), 2)) } else { next };
        prev = enter(net, next);
        if slot(next) == 0 && slot(prev) == 0 && node(prev) != 0 {
            stats.rules += 1;
            back = enter(net, port(node(prev), meta(net, node(prev))));
            rewrite(net, node(prev), node(next));
            next = enter(net, back);
        } else if slot(next) == 0 {
            warp.push(node(next));
            next = enter(net, port(node(next), 1));
        } else {
            set_meta(net, node(next), slot(next));
            next = enter(net, port(node(next), 0));
        }
        stats.loops += 1;
    }
    stats
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
fn thread_alg(net: &mut Net, _next: u32, _prev: u32, _back: u32, warp: &mut Vec<u32>, stats: &mut Stats, active: &mut u32) {
    let mut next = _next;
    let mut prev = _prev;
    let mut back = _back;
    loop {
        // wait on WARP_QUEUE semaphore
        // fetch ACTIVE mutex
        *active = *active + 1;
        // release ACTIVE mutex
        while (next > 0) || (warp.len() > 0) {
            // fetch  WARP_EDIT mutex
            if (next == 0) && (warp.len() > 0) {
                next = enter_port(net, port(warp.pop().unwrap(), 2));
            }
            // release WARP_EDIT mutex
            prev = enter_port(net, next);
            next = enter_port(net, prev);

            if get_port_slot(next) == 0 && get_port_slot(prev) == 0 && get_port_node(prev) != 0 {
                // fetch STATS mutex
                stats.rules = stats.rules + 1;
                // release STATS mutex
                back = enter_port(net, port(get_port_node(prev), get_node_meta(net, get_port_node(prev))));
                rewrite(net, get_port_node(prev), get_port_node(next));
                next = enter_port(net, back);
            } else if get_port_slot(next) == 0 {
                // fetch WARP_EDIT mutex
                warp.push(get_port_node(next));
                // release WARP_EDIT semaphore
                // signal WARP_QUEUE semaphore
                next = enter_port(net, port(get_port_node(next), 1));
            } else {
                // fetch NET_EDIT(node(next)) mutex
                set_node_meta(net, get_port_node(next), get_port_slot(next));
                next = enter_port(net, port(get_port_node(next), 0));
            }
            // fetch STATS mutex
            stats.loops = stats.loops + 1;
            // release STATS mutex
        }
        // fetch ACTIVE mutex
        *active = *active - 1;
        // release ACTIVE mutex
    }

}
