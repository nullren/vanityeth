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

type Selector = Box<dyn Fn(Address) -> bool>;

fn select_from_file<P>(filename: P) -> Selector
where
    P: AsRef<Path>,
{
    let prefixes = load_prefixes(filename);
    Box::new(move |addr| addr_matches_map(prefixes.clone(), addr))
}

fn addr_matches_map(prefixes: HashSet<String>, addr: Address) -> bool {
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

fn select_prefix(prefix: String) -> Selector {
    Box::new(move |addr| addr_has_prefix(addr, prefix.clone()))
}

fn addr_has_prefix(addr: Address, prefix: String) -> bool {
    let address = format!("{:?}", addr);
    &address[2..(2 + prefix.len())] == prefix
}

fn select_suffix(suffix: String) -> Selector {
    Box::new(move |addr| addr_has_suffix(addr, suffix.clone()))
}

fn addr_has_suffix(addr: Address, suffix: String) -> bool {
    let address = format!("{:?}", addr);
    &address[(42 - suffix.len())..42] == suffix
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
    /// Worker threads to use to generate wallets. The default value 0 means create as many workers as there are CPUs.
    #[structopt(short, long, default_value = "0")]
    workers: usize,
    /// Read address prefixes/suffixes from input file.
    #[structopt(parse(from_os_str), short, long)]
    input: Option<PathBuf>,
    /// Filter addresses with prefix
    #[structopt(short, long)]
    prefix: Option<String>,
    /// Filter addresses with suffix
    #[structopt(short, long)]
    suffix: Option<String>,
}

fn main() {
    let args: Cli = Cli::from_args();
    let workers = if args.workers < 1 {
        num_cpus::get() - 1
    } else {
        args.workers
    };

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
        for mnemonic in mnemonics {
            if !r.load(Ordering::Relaxed) {
                break;
            }
            let m_tx = m_tx.clone();
            m_tx.send(mnemonic).unwrap();
        }
        drop(m_tx);
    });

    // workers and wallet generators
    let (w_tx, w_rx) = std::sync::mpsc::sync_channel(workers + 1);
    eprintln!("starting {} worker threads", workers);
    for id in 0..workers {
        w_tx.send(id).unwrap();
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    thread::spawn(move || {
        for (worker, mnemonic) in w_rx.iter().zip(m_rx) {
            let w_tx = w_tx.clone();
            let tx = tx.clone();
            thread::spawn(move || {
                let wallet = to_wallet(mnemonic).unwrap();
                tx.send(wallet).unwrap();
                // w_tx gets dropped when m_tx is closed
                if let Err(e) = w_tx.send(worker) {
                    if format!("{}", e) != "sending on a closed channel" {
                        eprintln!("cannot send worker: {}", e);
                    }
                }
            });
        }
    });

    // filter wallets
    let select = if let Some(filename) = args.input {
        select_from_file(filename)
    } else if let Some(suffix) = args.suffix {
        select_suffix(suffix)
    } else if let Some(prefix) = args.prefix {
        select_prefix(prefix)
    } else {
        Box::new(|_| true)
    };

    let mut count = 0u32;
    let mut start = Instant::now();
    for (addr, phrase) in rx {
        count += 1;
        if count % 1000 == 0 {
            eprint!(".");
        }
        if select(addr) {
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
    let duration = start.elapsed();
    eprintln!(
        "workers terminated; {:.2} wallets per second checked",
        (count as f64) / duration.as_secs_f64()
    );
}
