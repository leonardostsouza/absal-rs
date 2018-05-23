pub mod term;
pub mod net;
pub mod simple_semaphore;

pub fn reduce(code : &str) -> (net::Stats, String) {
    let term = term::from_string(code.as_bytes());
    //let net = term::to_net(&term);

    let locks = net::Locks::new(term::to_net(&term));
    println!("Iniciating reduce()");
    let resp = net::reduce(locks);

    //let stats = net::reduce(&net);
    let net = resp.1;
    let stats = resp.0;
    let reduced_term = term::from_net(&net);
    let reduced_code = term::to_string(&reduced_term);

    (stats, String::from_utf8(reduced_code).unwrap())
}
