extern crate core;

use std::collections::btree_map::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Result;

fn main() {
    print_an_address().unwrap();
}

#[derive(Serialize, Deserialize, Debug)]
struct Address {
    street: String,
    city: Vec<String>,
    name: Option<Box<Address>>,
}
#[derive(Serialize, Deserialize, Debug)]
struct A {
    map: BTreeMap<String, Vec<u8>>,
}

#[derive(Serialize, Deserialize, Debug)]
enum Data {
    A(A),
    B(Address),
}

fn print_an_address() -> Result<()> {
    // Some data structure.
    let address = Address {
        street: "10 Downing Street".to_owned(),
        city: vec!["London".to_owned(), "New York".to_owned()],
        name: Some(Box::new(Address {
            street: "5 Sec".to_string(),
            city: vec!["Beijing".to_string()],
            name: None,
        })),
    };

    // Serialize it to a JSON string.
    let j = serde_json::to_vec(&address)?;

    // Print, write to a file, or send to an HTTP server.
    println!("{j:?}");

    let address: Address = serde_json::from_slice(&j)?;
    println!("{address:?}");

    let mut a = A {
        map: BTreeMap::new(),
    };
    a.map.insert("a".to_string(), vec![1, 2, 3]);

    let data = Data::A(a);
    let j = serde_json::to_string(&data)?;
    println!("{j:?}");
    let data: Data = serde_json::from_str(&j)?;
    println!("{data:?}");

    let x = u32::MAX; // 4字节

    println!("{:08x}", 1);
    println!("{:?}", 12u32.to_be_bytes());
    println!("{:?}", 10u32.to_be_bytes());
    println!("{:?}", x.to_be_bytes());

    Ok(())
}
