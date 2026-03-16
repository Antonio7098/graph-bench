use graphbench_core::graph::persist_codegraph_snapshot;
use std::env;

fn main() {
    let mut args = env::args().skip(1);
    let Some(repository_root) = args.next() else {
        eprintln!("usage: generate_snapshot <repository_root> <commit_hash> <destination>");
        std::process::exit(1);
    };
    let Some(commit_hash) = args.next() else {
        eprintln!("usage: generate_snapshot <repository_root> <commit_hash> <destination>");
        std::process::exit(1);
    };
    let Some(destination) = args.next() else {
        eprintln!("usage: generate_snapshot <repository_root> <commit_hash> <destination>");
        std::process::exit(1);
    };

    match persist_codegraph_snapshot(repository_root, &commit_hash, destination) {
        Ok(hash) => println!("{hash}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
