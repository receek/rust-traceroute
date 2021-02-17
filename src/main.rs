

use std::net::Ipv4Addr;
use std::str::FromStr;
use std::env;

mod tracroute;
use tracroute::run_traceroute;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Usage: traceroute <IPv4 host>", );
        return;
    }

    let ip_a;

    match Ipv4Addr::from_str(args[1].as_str()) {
        Ok(parsed) => ip_a = parsed,
        Err(err) => {
            println!("{}", err);
            return;
        },
    }

    run_traceroute(ip_a, 5, 1);
}
