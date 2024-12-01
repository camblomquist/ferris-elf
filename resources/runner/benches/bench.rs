use iai_callgrind::{library_benchmark, library_benchmark_group, main, LibraryBenchmarkConfig};

use std::env;
use std::hint::black_box;

pub trait IntoInput<T: Copy> {
    fn into_input(self) -> T;
}

impl IntoInput<&[u8]> for Vec<u8> {
    fn into_input(self) -> &'static [u8] {
        self.leak()
    }
}

impl IntoInput<&str> for Vec<u8> {
    fn into_input(self) -> &'static str {
        unsafe { std::str::from_utf8_unchecked(self.leak()) }
    }
}

fn read_input(name: &str) -> Vec<u8> {
    env::var(name).unwrap().into_bytes()
}

fn print_result(res: i64) {
    println!("Solution: {res}");
}

#[library_benchmark]
#[benches::run(args = ["INPUT_1", "INPUT_2", "INPUT_3"], setup = read_input, teardown = print_result)]
fn bench_run(input: Vec<u8>) -> i64 {
    black_box(runner::run(input.into_input()))
}

library_benchmark_group!(
    name = group;
    benchmarks = bench_run
);
main!(
    config = LibraryBenchmarkConfig::with_callgrind_args(["--cache-sim=no"]);
    library_benchmark_groups = group
);
