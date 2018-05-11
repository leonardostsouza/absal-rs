#![allow(dead_code)]

use lambda_calculus::{*};
use self::Term::{*};

/*
 * Variables:
 *      active - Couter wich shows how many secondary threads are active
 *
 * Semaphores:
 *      WARP_QUEUE - says if there is any information in the "warp" vector
 *
 * Mutexes:
 *      WARP_EDIT
 *      NET_EDIT - For now, only 1 for the whole graph. Can be changed to many in the future
 *      NET_REUSE
 *      STATS
 *      ACTIVE - Mutex to increment or decrement the "active" variable
 *
 * Observations:
 *      - Probably we are going to need another technique to prevent threads from
 *        reading while others are writing
 */


#[derive(Clone, Debug)]
pub struct Stats {
    loops: u32,
    rules: u32,
    betas: u32,
    dupls: u32,
    annis: u32
}

#[derive(Clone, Debug)]
pub struct Net {
    pub nodes: Vec<u32>,
    pub reuse: Vec<u32>
}

type Port = u32;

fn new_node(net : &mut Net, kind : u32) -> u32 {
    // fetch NET_REUSE mutex
    let reuse = net.reuse.pop();
    // release NET_REUSE mutex

    let node : u32 = match reuse {
        Some(index) => index,
        None        => {
            let len = net.nodes.len();
            // fetch NET_EDIT mutex
            net.nodes.resize(len + 4, 0);
            // release NET_EDIT mutex
            (len as u32) / 4
        }
    };

    // fetch NET_EDIT mutex
    net.nodes[(node * 4 + 0) as usize] = node * 4 + 0;
    net.nodes[(node * 4 + 1) as usize] = node * 4 + 1;
    net.nodes[(node * 4 + 2) as usize] = node * 4 + 2;
    net.nodes[(node * 4 + 3) as usize] = kind << 2;
    // release NET_EDIT mutex

    return node;
}

fn port(node : u32, slot : u32) -> Port {
    (node << 2) | slot
}

fn get_port_node(port : Port) -> u32 {
    port >> 2
}

fn get_port_slot(port : Port) -> u32 {
    port & 3
}

fn enter_port(net : &Net, port : Port) -> Port {
    net.nodes[port as usize]
}

fn get_node_kind(net : &Net, node_index : u32) -> u32 {
    net.nodes[(node_index * 4 + 3) as usize] >> 2
}

fn get_node_meta(net : &Net, node_index : u32) -> u32 {
    net.nodes[(node_index * 4 + 3) as usize] & 3
}

fn set_node_meta(net : &mut Net, node_index : u32, meta : u32) {
    let ptr = (node_index * 4 + 3) as usize;
    net.nodes[ptr] = net.nodes[ptr] & 0xFFFFFFFC | meta;
}

fn link(net : &mut Net, ptr_a : u32, ptr_b : u32) {
    net.nodes[ptr_a as usize] = ptr_b;
    net.nodes[ptr_b as usize] = ptr_a;
}

pub fn to_net(term : &Term) -> Net {
    fn encode(net : &mut Net, kind : &mut u32, scope : &mut Vec<u32>, term : &Term) -> Port {
        match term {
            &App{ref fun, ref arg} => {
                let app = new_node(net, 1);
                let fun = encode(net, kind, scope, fun);
                link(net, port(app, 0), fun);
                let arg = encode(net, kind, scope, arg);
                link(net, port(app, 1), arg);
                port(app, 2)
            },
            &Lam{ref bod} => {
                let fun = new_node(net, 1);
                let era = new_node(net, 0);
                link(net, port(fun, 1), port(era, 0));
                link(net, port(era, 1), port(era, 2));
                scope.push(fun);
                let bod = encode(net, kind, scope, bod);
                scope.pop();
                link(net, port(fun, 2), bod);
                port(fun, 0)
            },
            &Var{ref idx} => {
                let lam = scope[scope.len() - 1 - (*idx as usize)];
                if get_node_kind(net, get_port_node(enter_port(net, port(lam, 1)))) == 0 {
                    port(lam, 1)
                } else {
                    *kind = *kind + 1;
                    let dup = new_node(net, *kind);
                    let arg = enter_port(net, port(lam, 1));
                    link(net, port(dup, 1), arg);
                    link(net, port(dup, 0), port(lam, 1));
                    port(dup, 2)
                }
            }
        }
    }
    let mut net : Net = Net { nodes: vec![0,1,2,0], reuse: vec![] };
    let mut kind : u32 = 1;
    let mut scope : Vec<u32> = Vec::new();
    let ptr : Port = encode(&mut net, &mut kind, &mut scope, term);
    link(&mut net, 0, ptr);
    net
}

pub fn from_net(net : &Net) -> Term {
    fn go(net : &Net, node_depth : &mut Vec<u32>, next : Port, exit : &mut Vec<Port>, depth : u32) -> Term {
        let prev_port = enter_port(net, next);
        let prev_slot = get_port_slot(prev_port);
        let prev_node = get_port_node(prev_port);
        //println!("{} {:?} {} {} {} {}", next, exit, depth, prev_port, prev_slot, prev_node);
        if get_node_kind(net, prev_node) == 1 {
            match prev_slot {
                0 => {
                    node_depth[prev_node as usize] = depth;
                    Lam{bod: Box::new(go(net, node_depth, port(prev_node, 2), exit, depth + 1))}
                },
                1 => {
                    Var{idx: depth - node_depth[prev_node as usize] - 1}
                },
                _ => {
                    let fun = go(net, node_depth, port(prev_node, 0), exit, depth);
                    let arg = go(net, node_depth, port(prev_node, 1), exit, depth);
                    App{fun: Box::new(fun), arg: Box::new(arg)}
                }
            }
        } else if prev_slot > 0 {
            exit.push(prev_slot);
            let term = go(net, node_depth, port(prev_node, 0), exit, depth);
            exit.pop();
            term
        } else {
            let e = exit.pop().unwrap();
            let term = go(net, node_depth, port(prev_node, e), exit, depth);
            exit.push(e);
            term
        }
    }
    let mut node_depth : Vec<u32> = Vec::with_capacity(net.nodes.len() / 4);
    let mut exit : Vec<u32> = Vec::new();
    node_depth.resize(net.nodes.len() / 4, 0);
    go(net, &mut node_depth, 0, &mut exit, 0)
}

pub fn reduce(net : &mut Net) -> Stats {
    let mut stats = Stats { loops: 0, rules: 0, betas: 0, dupls: 0, annis: 0 };
    let mut warp: Vec<u32> = Vec::new();
    let mut next : Port = net.nodes[0];
    let mut prev : Port;
    let mut back : Port;
    while (next > 0) || (warp.len() > 0) || (active > 0) {
        if (next == 0) {
            if (warp.len() == 0) {
                continue;
            } else {
                // fetch WARP_EDIT mutex
                let warp_node = warp.pop();
                // release WARP_EDIT mutex
                next = match warp_node {
                    Some(index) => enter_port(net, port(index, 2)),
                    None        => continue
                };
            }
        }
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
            // release WARP_EDIT Mutex
            // signal WARP_QUEUE semaphore
            next = enter_port(net, port(get_port_node(next), 1));
        } else {
            // fetch NET_EDIT mutex
            set_node_meta(net, get_port_node(next), get_port_slot(next));
            // release NET_EDIT mutex
            next = enter_port(net, port(get_port_node(next), 0));
        }

        stats.loops = stats.loops + 1;
    }
    stats
}

fn rewrite(net : &mut Net, x : Port, y : Port) {
    if get_node_kind(net, x) == get_node_kind(net, y) {
        let px1 = enter_port(net, port(x, 1));
        let py1 = enter_port(net, port(y, 1));

        let px2 = enter_port(net, port(x, 2));
        let py2 = enter_port(net, port(y, 2));

        // fetch NET_EDIT mutex
        link(net, px1, py1);
        link(net, px2, py2);
        // release NET_EDIT mutex

        // fetch NET_REUSE mutex
        net.reuse.push(x);
        net.reuse.push(y);
        // release NET_REUSE mutex

    } else {
        // add new nodes
        let t = get_node_kind(net, x);
        let a = new_node(net, t);
        let t = get_node_kind(net, y);
        let b = new_node(net, t);


        let px1 = enter_port(net, port(x, 1));
        let px2 = enter_port(net, port(x, 2));
        let py1 = enter_port(net, port(y, 1));
        let py2 = enter_port(net, port(y, 2));

        // fetch NET_EDIT mutex
        link(net, port(b, 0), px1);
        link(net, port(y, 0), px2);
        link(net, port(a, 0), py1);
        link(net, port(x, 0), py2);

        link(net, port(a, 1), port(b, 1));
        link(net, port(a, 2), port(y, 1));
        link(net, port(x, 1), port(b, 2));
        link(net, port(x, 2), port(y, 2));

        set_node_meta(net, x, 0);
        set_node_meta(net, y, 0);
        // release NET_EDIT mutex
    }
}

fn thread_alg(net: &mut Net, next: u32, prev: u32, back: u32, warp: &mut Vec<u32>, stats: &mut Stats, active: &mut u32) {
    while(true){
        // wait on WARP_QUEUE semaphore
        // fetch ACTIVE mutex
        active = active + 1;
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
        active = active - 1;
        // release ACTIVE mutex
    }

}
