// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use clap::*;
use tracing::info;

use sui_sdk::crypto::SuiKeystore;
use sui_types::base_types::SuiAddress;
use sui_types::base_types::{decode_bytes_hex, encode_bytes_hex};
use sui_types::crypto::{
    random_key_pair_by_type, AuthorityKeyPair, EncodeDecodeBase64, SignatureScheme, SuiKeyPair,
};
use sui_types::sui_serde::{Base64, Encoding};

#[cfg(test)]
#[path = "unit_tests/keytool_tests.rs"]
mod keytool_tests;

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
#[clap(rename_all = "kebab-case")]
pub enum KeyToolCommand {
    /// Generate a new keypair with keypair scheme flag {ed25519 | secp256k1}. Output file to current dir (to generate keypair to sui.keystore, use `sui client new-address`)
    Generate {
        key_scheme: SignatureScheme,
    },
    Show {
        file: PathBuf,
    },
    /// Extract components of a base64-encoded keypair to reveal the Sui address, public key, and key scheme flag.
    Unpack {
        keypair: SuiKeyPair,
    },
    /// List all keys by its address, public key, key scheme in the keystore
    List,
    /// Create signature using the sui keystore and provided data.
    Sign {
        #[clap(long, parse(try_from_str = decode_bytes_hex))]
        address: SuiAddress,
        #[clap(long)]
        data: String,
    },
    /// Import mnemonic phrase and generate keypair based on key scheme flag {ed25519 | secp256k1}.
    Import {
        mnemonic_phrase: String,
        key_scheme: SignatureScheme,
    },
    /// This is a temporary helper function to ensure that testnet genesis does not break while
    /// we transition towards BLS signatures.
    LoadKeypair {
        file: PathBuf,
    },
}

impl KeyToolCommand {
    pub fn execute(self, keystore: &mut SuiKeystore) -> Result<(), anyhow::Error> {
        match self {
            KeyToolCommand::Generate { key_scheme } => {
                let k = key_scheme.to_string();
                match random_key_pair_by_type(key_scheme) {
                    Ok((address, keypair)) => {
                        let file_name = format!("{address}.key");
                        write_keypair_to_file(&keypair, &file_name)?;
                        println!("{:?} key generated and saved to '{file_name}'", k);
                    }
                    Err(e) => {
                        println!("Failed to generate keypair: {:?}", e)
                    }
                }
            }

            KeyToolCommand::Show { file } => {
                let res: Result<SuiKeyPair, anyhow::Error> = read_keypair_from_file(&file);
                match res {
                    Ok(keypair) => {
                        println!("Public Key: {}", encode_bytes_hex(keypair.public()));
                        println!("Flag: {}", keypair.public().flag());
                    }
                    Err(e) => {
                        println!("Failed to read keypair at path {:?} err: {:?}", file, e)
                    }
                }
            }

            KeyToolCommand::Unpack { keypair } => {
                store_and_print_keypair((&keypair.public()).into(), keypair)
            }
            KeyToolCommand::List => {
                println!(
                    " {0: ^42} | {1: ^45} | {2: ^6}",
                    "Sui Address", "Public Key (Base64)", "Scheme"
                );
                println!("{}", ["-"; 100].join(""));
                for pub_key in keystore.keys() {
                    println!(
                        " {0: ^42} | {1: ^45} | {2: ^6}",
                        Into::<SuiAddress>::into(&pub_key),
                        Base64::encode(&pub_key),
                        pub_key.scheme().to_string()
                    );
                }
            }
            KeyToolCommand::Sign { address, data } => {
                info!("Data to sign : {}", data);
                info!("Address : {}", address);
                let message = Base64::decode(&data).map_err(|e| anyhow!(e))?;
                let signature = keystore.sign(&address, &message)?;
                // Separate pub key and signature string, signature and pub key are concatenated with an '@' symbol.
                let signature_string = format!("{:?}", signature);
                let sig_split = signature_string.split('@').collect::<Vec<_>>();
                let flag = sig_split
                    .first()
                    .ok_or_else(|| anyhow!("Error creating signature."))?;
                let signature = sig_split
                    .get(1)
                    .ok_or_else(|| anyhow!("Error creating signature."))?;
                let pub_key = sig_split
                    .last()
                    .ok_or_else(|| anyhow!("Error creating signature."))?;
                info!("Flag Base64: {}", flag);
                info!("Public Key Base64: {}", pub_key);
                info!("Signature : {}", signature);
            }
            KeyToolCommand::Import {
                mnemonic_phrase,
                key_scheme,
            } => {
                let address = keystore.import_from_mnemonic(&mnemonic_phrase, key_scheme)?;
                info!("Key imported for address [{address}]");
            }

            KeyToolCommand::LoadKeypair { file } => {
                let res: Result<SuiKeyPair, anyhow::Error> = read_keypair_from_file(&file);

                match res {
                    Ok(keypair) => {
                        println!("Account Keypair: {}", keypair.encode_base64());
                        println!("Network Keypair: {}", keypair.encode_base64());
                        if let SuiKeyPair::Ed25519SuiKeyPair(kp) = keypair {
                            println!("Protocol Keypair: {}", kp.encode_base64());
                        };
                    }
                    Err(e) => {
                        println!("Failed to read keypair at path {:?} err: {:?}", file, e)
                    }
                }
            }
        }

        Ok(())
    }
}

fn store_and_print_keypair(address: SuiAddress, keypair: SuiKeyPair) {
    let path_str = format!("{}.key", address).to_lowercase();
    let path = Path::new(&path_str);
    let address = format!("{}", address);
    let kp = keypair.encode_base64();
    let flag = keypair.public().flag();
    let out_str = format!("address: {}\nkeypair: {}\nflag: {}", address, kp, flag);
    fs::write(path, out_str).unwrap();
    println!(
        "Address, keypair and key scheme written to {}",
        path.to_str().unwrap()
    );
}

pub fn write_keypair_to_file<P: AsRef<std::path::Path>>(
    keypair: &SuiKeyPair,
    path: P,
) -> anyhow::Result<()> {
    let contents = keypair.encode_base64();
    std::fs::write(path, contents)?;
    Ok(())
}

pub fn read_authority_keypair_from_file<P: AsRef<std::path::Path>>(
    path: P,
) -> anyhow::Result<AuthorityKeyPair> {
    match read_keypair_from_file(path) {
        Ok(kp) => match kp {
            SuiKeyPair::Ed25519SuiKeyPair(k) => Ok(k),
            SuiKeyPair::Secp256k1SuiKeyPair(_) => Err(anyhow!("Invalid authority keypair type")),
        },
        Err(e) => Err(anyhow!("Failed to read keypair file {:?}", e)),
    }
}

pub fn read_keypair_from_file<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<SuiKeyPair> {
    let contents = std::fs::read_to_string(path)?;
    SuiKeyPair::decode_base64(contents.as_str().trim()).map_err(|e| anyhow!(e))
}
