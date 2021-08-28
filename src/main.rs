use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time::Instant;

use ethers::prelude::Signer;
use ethers::signers::{
    coins_bip39::{English, Mnemonic},
    MnemonicBuilder,
};
use ethers::types::Address;
use rand::thread_rng;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use structopt::StructOpt;

fn to_wallet(mnemonic: Mnemonic<English>) -> Result<(Address, String), Box<dyn std::error::Error>> {
    let phrase: &str = &mnemonic.to_phrase()?;
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(phrase)
        .build()?;
    Ok((wallet.address(), phrase.to_owned()))
}

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn load_prefixes<P>(filename: P) -> HashSet<String>
where
    P: AsRef<Path>,
{
    let mut prefixes: HashSet<String> = HashSet::new();
    if let Ok(lines) = read_lines(filename) {
        // Consumes the iterator, returns an (Optional) String
        for line in lines {
            if let Ok(word) = line {
                if word.len() != 4 {
                    panic!(
                        "input file must contain only 4 letter words: {:?} is {} characters",
                        word,
                        word.len()
                    )
                }
                prefixes.insert(word);
            }
        }
    }
    eprintln!("read {} prefixes", prefixes.len());
    return prefixes;
}

fn select_address(prefixes: &HashSet<String>, addr: Address) -> bool {
    let address = format!("{:?}", addr);
    // ¯\_(ツ)_/¯
    (prefixes.contains(&address[2..6]) && prefixes.contains(&address[38..42]))
        || (&address[2..3] == &address[3..4]
            && &address[2..3] == &address[4..5]
            && &address[2..3] == &address[5..6]
            && &address[2..3] == &address[38..39]
            && &address[2..3] == &address[39..40]
            && &address[2..3] == &address[40..41]
            && &address[2..3] == &address[41..42])
}

struct Mnemonics {}

impl Mnemonics {
    fn new() -> Mnemonics {
        Mnemonics {}
    }
}

impl Iterator for Mnemonics {
    type Item = Mnemonic<English>;

    fn next(&mut self) -> Option<Self::Item> {
        match Mnemonic::new_with_count(&mut thread_rng(), 12) {
            Ok(mnemonic) => Some(mnemonic),
            Err(e) => {
                panic!("{:?}", e);
            }
        }
    }
}

#[derive(StructOpt)]
struct Cli {
    /// Worker threads to use to generate wallets
    #[structopt(short, long, default_value = "10")]
    workers: usize,
    /// Read address prefixes/suffixes from input file
    #[structopt(parse(from_os_str), short, long)]
    input: PathBuf,
}

fn main() {
    let args: Cli = Cli::from_args();
    let workers = args.workers;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("stopping search");
        r.store(false, Ordering::Relaxed);
    })
    .expect("could not add ctrl-c handler");

    // produce mnemonics
    let (m_tx, m_rx) = std::sync::mpsc::sync_channel(workers);
    let r = running.clone();
    thread::spawn(move || {
        let mnemonics = Mnemonics::new();
        let mut count = 0u32;
        for mnemonic in mnemonics {
            if !r.load(Ordering::Relaxed) {
                break;
            }
            let m_tx = m_tx.clone();
            count += 1;
            if count % 1000 == 0 {
                eprint!("m");
            }
            m_tx.send(mnemonic).unwrap();
        }
        eprintln!("stopping mnemonic generator");
        drop(m_tx);
    });

    // workers and wallet generators
    let (w_tx, w_rx) = std::sync::mpsc::sync_channel(workers + 1);
    for id in 0..workers {
        w_tx.send(id).unwrap();
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut count = 0u32;
        for (worker, mnemonic) in w_rx.iter().zip(m_rx) {
            count += 1;
            if count % 1000 == 0 {
                eprint!("w");
            }
            let w_tx = w_tx.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let wallet = to_wallet(mnemonic).unwrap();
                if let Err(e) = tx.send(wallet) {
                    eprintln!("cannot send address: {}", e);
                }
                if let Err(e) = w_tx.send(worker) {
                    eprintln!("cannot send worker: {}", e);
                }
            });
        }
        eprintln!("stopping workers");
    });

    // filter wallets
    let prefixes = load_prefixes(args.input);
    let mut count = 0u32;
    let mut start = Instant::now();
    for (addr, phrase) in rx {
        count += 1;
        if count % 1000 == 0 {
            eprint!("a");
        }
        if select_address(&prefixes, addr) {
            eprintln!();
            let duration = start.elapsed();
            eprintln!(
                "{} wallets since last match; {:.2} wallets per second checked",
                count,
                (count as f64) / duration.as_secs_f64()
            );
            println!("{}: {}", addr, phrase);
            count = 0;
            start = Instant::now();
        }
    }
    eprintln!("done");
}
