#![allow(dead_code)]

use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time;
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

// !! THREAD UNSAFE !!
pub fn new_node(net : &mut Net, kind : u32) -> u32 {
    let node : u32 = match net.reuse.pop() {
        Some(index) => index,
        None        => {
            let len = net.nodes.len();
            net.nodes.resize(len + 4, 0);
            (len as u32) / 4
        }
    };
    net.nodes[port(node, 0) as usize] = port(node, 0);
    net.nodes[port(node, 1) as usize] = port(node, 1);
    net.nodes[port(node, 2) as usize] = port(node, 2);
    net.nodes[port(node, 3) as usize] = kind << 2;
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

// !! THREAD UNSAFE !!
pub fn enter(net : &Net, port : Port) -> Port {
    net.nodes[port as usize]
}

// !! THREAD UNSAFE !!
pub fn kind(net : &Net, node : u32) -> u32 {
    net.nodes[port(node, 3) as usize] >> 2
}

// !! THREAD UNSAFE !!
pub fn meta(net : &Net, node : u32) -> u32 {
    net.nodes[port(node, 3) as usize] & 3
}

// !! THREAD UNSAFE !!
pub fn set_meta(net : &mut Net, node : u32, meta : u32) {
    let ptr = port(node, 3) as usize;
    net.nodes[ptr] = net.nodes[ptr] & 0xFFFFFFFC | meta;
}

// !! THREAD UNSAFE !!
pub fn link(net : &mut Net, ptr_a : u32, ptr_b : u32) {
    net.nodes[ptr_a as usize] = ptr_b;
    net.nodes[ptr_b as usize] = ptr_a;
}


pub fn reduce(_locks : Locks) -> (Stats, Net) {
    let locks = Arc::new(_locks);
    let net = locks.net.read().unwrap();
    let mut next : Port = net.nodes[0];
    drop(net);
    let mut prev : Port = 0;
    let mut back : Port = 0;
    let mut handles = vec![];

    println!("Creating threads");
    // spawn threads
    for _ in 0..NTHREADS {
        let locks = Arc::clone(&locks);
        handles.push(thread::spawn (move || {
            thread_alg(locks);
        }));
    }

    println!("Entering first while loop");
    while next > 0 {
        //println!("next = {:?}", next);
        reduce_iteration(&locks, &mut next, &mut prev, &mut back);
    }

    // WAIT FOR THREADS TO FINISH
    thread::sleep(time::Duration::new(3, 0));
    /*for handle in handles {
        let _ = handle.join();
    }*/

    let stats = locks.stats.lock().unwrap().clone();
    let net = locks.net.read().unwrap().clone();
    (stats, net)
    //stats
}

/*
pub fn rewrite(locks : &Arc<Locks>, x : Port, y : Port) {
    println!("Rewrite!");
    // acquire NET_READ RwLock
    let net = locks.net.read().unwrap();
    let kx = kind(&net, x);
    let ky = kind(&net, y);
    drop(net);
    println!("\tkx = {:?}; ky = {:?}", kx, ky);
    // NET_READ RwLock released

    if kx == ky {
        println!("\tRewrite if...");
        // acquire NET_READ RwLock
        //let net = locks.net.read().unwrap();
        let mut net = locks.net.write().unwrap();
        /*let px1 = enter(&net, port(x, 1));
        let py1 = enter(&net, port(y, 1));
        let px2 = enter(&net, port(x, 2));
        let py2 = enter(&net, port(y, 2));
        //drop(net);
        // NET_READ RwLock released

        println!("\tpx1 = {:?}; px2 = {:?}; py1 = {:?}; py2 = {:?}", px1, px2, py1, py2);

        // acquire NET_WRITE RwLock
        //let mut net = locks.net.write().unwrap();
        link(&mut net, px1, py1);
        link(&mut net, px2, py2);*/

        let p0 = enter(&net, port(x, 1));
        let p1 = enter(&net, port(y, 1));
        link(&mut net, p0, p1);
        let p0 = enter(&net, port(x, 2));
        let p1 = enter(&net, port(y, 2));
        link(&mut net, p0, p1);

        net.reuse.push(x);
        net.reuse.push(y);
        // NET_WRITE RwLock released

    } else {
        println!("\tRewrite else...");
        // acquire NET_READ RwLock
        let net = locks.net.read().unwrap();
        let tx = kind(&net, x);
        let ty = kind(&net, y);

        let px1 = enter(&net, port(x, 1));
        let px2 = enter(&net, port(x, 2));
        let py1 = enter(&net, port(y, 1));
        let py2 = enter(&net, port(y, 2));
        drop(net);
        // NET_READ RwLock released

        // acquire NET_WRITE RwLock
        let mut net = locks.net.write().unwrap();
        let a = new_node(&mut net, tx);
        let b = new_node(&mut net, ty);

        link(&mut net, port(b, 0), px1);
        link(&mut net, port(y, 0), px2);
        link(&mut net, port(a, 0), py1);
        link(&mut net, port(x, 0), py2);

        link(&mut net, port(a, 1), port(b, 1));
        link(&mut net, port(a, 2), port(y, 1));
        link(&mut net, port(x, 1), port(b, 2));
        link(&mut net, port(x, 2), port(y, 2));

        set_meta(&mut net, x, 0);
        set_meta(&mut net, y, 0);
        // NET_WRITE RwLock released
    }
}*/

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
        let a = new_node(net, t);
        let t = kind(net, y);
        let b = new_node(net, t);
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


fn thread_alg(locks: Arc<Locks>) {
    println!("========THREAD STARTED!!");
    let mut next : Port = 0;
    let mut prev : Port = 0;
    let mut back : Port = 0;
    loop {
        // wait on WARP_QUEUE semaphore
        locks.warp_queue.wait();
        println!("========THREAD Working....");

        // fetch ACTIVE mutex
        let mut active = locks.active_threads.lock().unwrap();
        *active += 1;
        drop(active);
        // release ACTIVE mutex

        // "Black Magic" do-while loop
        while {
            reduce_iteration(&locks, &mut next, &mut prev, &mut back);
            next > 0
        }{}

        // fetch ACTIVE mutex
        let mut active = locks.active_threads.lock().unwrap();
        *active -= 1;
        // release ACTIVE mutex
    }

}


fn reduce_iteration(locks: &Arc<Locks>, next: &mut u32, prev: &mut u32, back: &mut u32) {

    //let mut net = locks.net.write().unwrap();
        *next = if *next == 0 {
            let index = {
                //acquire WARP Mutex
                let mut warp = locks.warp.lock().unwrap();
                warp.pop().unwrap()
                // WARP mutex released
            };
            let net = locks.net.read().unwrap();
            enter(&net, port(index, 2))
        }
        else {
            *next
        };
        //acquire NET_READ Mutex
        let net = locks.net.read().unwrap();
        *prev = enter(&net, *next);
        drop(net);
        // NET_READ Mutex released
        if slot(*next) == 0 && slot(*prev) == 0 && node(*prev) != 0 {
            ///////////// PROBLEMATIC CODE ///////////////
            let mut net = locks.net.write().unwrap();
            // acquire STATS mutex
            let mut stats = locks.stats.lock().unwrap();
            stats.rules += 1;
            drop(stats);
            // STATS mutex released

            // acquire NET_READ RwLock
            //let net = locks.net.read().unwrap();
            //let mut net = locks.net.write().unwrap();
            *back = enter(&net, port(node(*prev), meta(&net, node(*prev))));
            //drop(net);
            // NET_READ RwLock released

            //rewrite(&locks, node(*prev), node(*next)); //thread_safe
            rewrite(&mut net, node(*prev), node(*next));
            // acquire NET_READ RwLock
            //let net = locks.net.read().unwrap();
            *next = enter(&net, *back);
            // NET_READ RwLock released
            ///////////// END PROBLEMATIC CODE ///////////////
        }
        else if slot(*next) == 0 {
            // aquire WARP mutex
            let mut warp = locks.warp.lock().unwrap();
            warp.push(node(*next));
            drop(warp);
            // WARP mutex released

            // signal WARP_QUEUE semaphore
            locks.warp_queue.signal();

            // acquire NET_READ RwLock
            let net = locks.net.read().unwrap();
            *next = enter(&net, port(node(*next), 1));
            // NET_READ RwLock released
        }
        else {
            // acquire NET_WRITE RwLock
            let mut net = locks.net.write().unwrap();
            set_meta(&mut net, node(*next), slot(*next));
            drop(net);
            // NET_WRITE RwLock released

            // acquire NET_READ RwLock
            let net = locks.net.read().unwrap();
            *next = enter(&net, port(node(*next), 0));
            // NET_READ RwLock released
        }
        // acquire STATS mutex
        let mut stats = locks.stats.lock().unwrap();
        stats.loops += 1;
        // STATS mutex released
}
