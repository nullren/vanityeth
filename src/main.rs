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
use structopt::StructOpt;

fn to_wallet(
    mnemonic: &Mnemonic<English>,
) -> Result<(Address, String), Box<dyn std::error::Error>> {
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
    let (tx, rx) = std::sync::mpsc::channel();

    let workers = args.workers;
    thread::spawn(move || {
        let mnemonics = Mnemonics::new();
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for mnemonic in mnemonics {
            let tx = tx.clone();
            pool.spawn(move || {
                let wallet = to_wallet(&mnemonic).unwrap();
                tx.send(wallet).unwrap();
            });
        }
    });

    let prefixes = load_prefixes(args.input);
    let mut count = 0u32;
    let mut start = Instant::now();
    for (addr, phrase) in rx.into_iter() {
        count += 1;
        if count % 1000 == 0 {
            eprint!(".");
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
}
