use std::io::{self, Write};

use ethers::prelude::{Signer};
use ethers::signers::{coins_bip39::{Mnemonic, English}, MnemonicBuilder};
use rand::{thread_rng};

fn generate(mnemonic: &Mnemonic<English>) -> Result<(String, String), Box<dyn std::error::Error>> {
    let phrase: &str = &mnemonic.to_phrase()?;

    let wallet = MnemonicBuilder::<English>::default()
      .phrase(phrase)
      .build()?;

    Ok((format!("{:?}", wallet.address()), phrase.to_owned()))
}

fn main() {
    let mut rng = thread_rng();
    let mut count = 0u32;
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
                if address.starts_with("0xdead") {
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
