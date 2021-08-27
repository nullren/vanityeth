use std::collections::HashSet;
use std::fs::File;
use std::io::{self, BufRead, Write};
use std::path::Path;

use ethers::prelude::Signer;
use ethers::signers::{coins_bip39::{English, Mnemonic}, MnemonicBuilder};
use rand::thread_rng;
use ethers::types::Address;

fn to_wallet(mnemonic: &Mnemonic<English>) -> Result<(Address, String), Box<dyn std::error::Error>> {
    let phrase: &str = &mnemonic.to_phrase()?;
    let wallet = MnemonicBuilder::<English>::default()
      .phrase(phrase)
      .build()?;
    Ok((wallet.address(), phrase.to_owned()))
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

fn select_address(prefixes: &HashSet<String>, addr: Address) -> bool {
    let address = format!("{:?}", addr);
    // ¯\_(ツ)_/¯
    (
        prefixes.contains(&address[2..6]) &&
          prefixes.contains(&address[38..42])
    ) ||
      (
          &address[2..3] == &address[3..4] &&
            &address[2..3] == &address[4..5] &&
            &address[2..3] == &address[5..6] &&
            &address[2..3] == &address[38..39] &&
            &address[2..3] == &address[39..40] &&
            &address[2..3] == &address[40..41] &&
            &address[2..3] == &address[41..42]
      )
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
        let wallet = to_wallet(&mnemonic);
        match wallet {
            Ok((addr, phrase)) => {
                if select_address(&prefixes, addr) {
                    eprintln!();
                    println!("{:?}: {}: ({}): {}", addr, phrase, addr, count);
                    count = 0;
                }
            },
            Err(e) => {
                println!("{:?}", e);
                break;
            }
        }
    }
}
