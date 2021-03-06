use structopt::StructOpt;
use std::process::{Command, Stdio};
use std::time::Instant;
use std::collections::HashMap;
use threadpool::ThreadPool;
use std::sync::mpsc::channel;

#[derive(StructOpt, PartialEq, Debug)]
struct Opt {
    #[structopt(short, required=false, default_value="1", help="Number of times to run a command")]
	repetitions: u32,
	#[structopt(short, required=false, default_value="1", help="Number of concurrent executions")]
	concurrency: u32,
    #[structopt(short, help="Pipe command stdout and stderr to /dev/null")]
	quiet: bool,
    #[structopt(short, help="Display a histogram")]
	histogram: bool,
    #[structopt(subcommand, help="Command to run")]
    command: Subcommands,
}

#[derive(StructOpt, PartialEq, Debug)]
enum Subcommands {
    #[structopt(external_subcommand)]
    Other(Vec<String>),
}

fn main() {
    let opt = Opt::from_args();
    let Subcommands::Other(cmd) = opt.command;
	let mut ticks = Vec::new();
	let pool = ThreadPool::new(opt.concurrency as usize);
	let (tx, rx) = channel();

    for _x in 0..opt.repetitions {
		let tx = tx.clone();
		let cmd = cmd.clone();
		let quiet = opt.quiet.clone();
		pool.execute(move || {
			let elapsed = run_command(&cmd, quiet);
			tx.send(elapsed).expect("Could not send to channel");
		})
	}
	
	drop(tx);
	for t in rx.iter() {
		let elapsed = t;
		ticks.push(elapsed);
	}

    ticks.sort();

    let mut sum = 0;
    let mut sum_square = 0;
    for tick in &mut ticks {
        sum += *tick;
        sum_square += *tick * *tick;
    }
    let min = ticks.first();
    let max = ticks.last();
    let avg = sum / opt.repetitions as u128;
    // Do I risk loosing some accuracy by casting to f64?
    let std_dev = ((sum_square / opt.repetitions as u128 - avg * avg) as f32).sqrt();

    let p95_index = 0.95 * opt.repetitions as f32 - 1.0;
    let p99_index = 0.99 * opt.repetitions as f32 - 1.0;

    let p95 = if p95_index == p95_index.round() {
        let i1 = ticks[p95_index as usize];
        let i2 = ticks[p95_index as usize + 1];
        (i1 + i2) / 2
    } else {
        ticks[p95_index.ceil() as usize] as u128
    };
    let p99 = if p99_index == p99_index.round() {
        let i1 = ticks[p99_index as usize];
        let i2 = ticks[p99_index as usize + 1];
        (i1 + i2) / 2
    } else {
        ticks[p99_index.ceil() as usize]
    };

    println!("Total time: {}ms", sum);
    println!("Repetitions: {}", opt.repetitions);
    println!("Average time: {}ms", avg);
    println!("Min: {}ms", min.unwrap());
    println!("Max: {}ms", max.unwrap());
    println!("Standard deviation: {}", std_dev);
    println!("p95: {}ms", p95);
    println!("p99: {}ms", p99);

    if opt.histogram {
        let rounding_quotient = match *min.unwrap() {
            0..=1_000 => 1,
            1_001..=10_000 => 10,
            10_001..=100_000 => 100,
            100_001..=1_000_000 => 1000,
            1_000_001..=std::u128::MAX => 10000,
        };
        let mut frequencies: HashMap<u128, u128> = HashMap::new();
        let mut max_freq = 0;
        for tick in &mut ticks {
            let rounded_time = *tick / rounding_quotient;
            let mut i = *frequencies.get(&rounded_time).unwrap_or(&0);
            i += 1;
            frequencies.insert(rounded_time, i);
            if i >= max_freq {
                max_freq = i;
            }
        }
        let mut histogram: HashMap<u128, u128> = HashMap::new();
        for (bin,count) in &mut frequencies {
            histogram.insert(*bin, *count);
        }

        let keys: Vec<&u128> = histogram.keys().collect::<Vec<&u128>>();
        let mut sorted_keys = Vec::new();
        for key in keys {
            sorted_keys.push(key);
        }
        sorted_keys.sort();
        println!("Histogram:");
        println!("time:	count	normalized bar");
        for rounded_time in sorted_keys {
            let count = histogram[rounded_time];
            let msecs = *rounded_time * rounding_quotient;
            let bars = "#".repeat((count * 40 / max_freq) as usize);
            println!("{}ms	{}	{}", msecs, count, bars)
        }
    }
}

fn run_command(cmd: &Vec<String>, quiet: bool) -> u128 {
    let now = Instant::now();
    let _output = if quiet {
        Command::new("sh")
            .arg("-c")
            .args(cmd)
            .stdout(Stdio::null()).stderr(Stdio::null())
            .output().expect("failed to execute process")
    } else {
        Command::new("sh")
            .arg("-c")
            .args(cmd)
            .stdout(Stdio::inherit()).stderr(Stdio::inherit())
            .output().expect("failed to execute process")
    };
    return now.elapsed().as_millis()
}
