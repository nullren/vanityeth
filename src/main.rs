use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::ops::Deref;
use std::path::Path;

use ethers::prelude::Signer;
use ethers::signers::{coins_bip39::{English, Mnemonic}, MnemonicBuilder};
use rand::thread_rng;

fn generate(mnemonic: &Mnemonic<English>) -> Result<(String, String), Box<dyn std::error::Error>> {
    let phrase: &str = &mnemonic.to_phrase()?;
    let wallet = MnemonicBuilder::<English>::default()
      .phrase(phrase)
      .build()?;
    Ok((format!("{:?}", wallet.address()), phrase.to_owned()))
}

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
    where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn load_prefixes() -> HashSet<String> {
    let mut prefixes: HashSet<String> = HashSet::new();
    if let Ok(lines) = read_lines("./ethaddrs.txt") {
        // Consumes the iterator, returns an (Optional) String
        for line in lines {
            if let Ok(ip) = line {
                prefixes.insert(ip);
            }
        }
    }
    eprintln!("read {} prefixes", prefixes.len());
    return prefixes;
}

fn main() {
    let mut rng = thread_rng();
    let mut count = 0u32;
    let prefixes = load_prefixes();
    loop {
        count += 1;
        if count % 100 == 0 {
            eprint!(".");
            io::stderr().flush().unwrap();
        }
        let mnemonic = Mnemonic::new_with_count(&mut rng, 12).unwrap();
        let wallet = generate(&mnemonic);
        match wallet {
            Ok((address, phrase)) => {
                // ¯\_(ツ)_/¯
                if prefixes.contains(&address[2..3]) ||
                  prefixes.contains(&address[2..4]) ||
                  prefixes.contains(&address[2..5]) ||
                  prefixes.contains(&address[2..6]) ||
                  prefixes.contains(&address[2..7]) ||
                  prefixes.contains(&address[2..8]) ||
                  prefixes.contains(&address[2..9]) ||
                  prefixes.contains(&address[2..10]) {
                    eprintln!();
                    println!("{}: {}", address, phrase);
                }
            },
            Err(e) => {
                println!("{:?}", e);
                break;
            }
        }
    }
}
