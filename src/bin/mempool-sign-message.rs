use std::process;
extern crate serde_json;
use serde_json::{json, Value};

fn print_version() {
    const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
    println!("v{}", VERSION.unwrap_or("unknown"));
    process::exit(0);
}
fn print_usage(code: i32) {
    println!("\n    Usage:\n");
    println!("\tsign_message <private_key> - print <public_key>\n");
    println!("\tsign_message <private_key> <string> - print signature of <string>");
    if code == 24 {
        println!("\t             ^private_key must be hexidecimal characters.");
    }
    if code == 64 {
        println!("\t             ^private_key must be 64 characters long.");
    }
    println!("\n    Example:\n");
    println!(
        "\tsign_message 0000000000000000000000000000000000000000000000000000000000000001 \"\""
    );
    if code == 999 {
        println!("\t                                       private_key must be greater than zero^");
    }
    println!("    Expected:\n");
    println!(
    "\t3044022077c8d336572f6f466055b5f70f433851f8f535f6c4fc71133a6cfd71079d03b702200ed9f5eb8aa5b266abac35d416c3207e7a538bf5f37649727d7a9823b1069577\n"
    );

    if code == 0 {
        process::exit(code);
    }
    if code == 64 {
        process::exit(code);
    }

    process::exit(0);
}

fn is_string_of_length_64(string: &str) -> bool {
    return string.len() == 64;
}

fn is_hex(text: &str) -> bool {
    use regex::Regex;
    let re = Regex::new(r"^[0-9a-fA-F]+$").unwrap();
    re.is_match(text)
}

//GLOBAL VARIABLES
//static GLOBAL_VAR_BOOL: &bool = &false;
//static GLOBAL_VAR_STRING: &str = "GLOBAL_VAR_STRING";
//END GLOBAL VARIABLES

fn main() -> Result<(), String> {
    let mut _verbose = false;
    use secp256k1::{Keypair, Scalar, Secp256k1, SecretKey, XOnlyPublicKey};
    use std::env;
    use std::str::FromStr;
    let secp = Secp256k1::new();
    let tweak = secp256k1::Scalar::random();

    let args: Vec<String> = env::args().collect();
    let _app_name = &args[0];

    // Create the JSON array
    let mut json_array = Vec::new();
    //let _num_args = args.len();
    //#[cfg(debug_assertions)]
    //println!("_num_args - 1 = {}", _num_args - 1);
    if env::args().len() == 1 {
        print_usage(0);
    }

    if env::args().len() > 1 {
        //begin handle args
        //begin handle args
        //begin handle args

        let private_key_arg = std::env::args()
            .nth(1)
            .expect("Missing private key argument");

        if is_hex(&private_key_arg) {
        } else {
            print_usage(24);
        }
        //0000000000000000000000000000000000000000000000000000000000000000
        if &private_key_arg == "0000000000000000000000000000000000000000000000000000000000000000" {
            //TODO:use as special case
            print_usage(999);
        }
        if &private_key_arg == "-vv" || &private_key_arg == "--verbose" {
            _verbose = true;
            println!("verbose={}", _verbose)
        }
        if &private_key_arg == "-h" || &private_key_arg == "--help" {
            print_usage(0);
        }
        if &private_key_arg == "-v" || &private_key_arg == "--version" || &private_key_arg == "-V" {
            print_version();
        }

        if is_string_of_length_64(&private_key_arg) {
        } else {
            print_usage(64);
        }

        let private_key = SecretKey::from_str(&private_key_arg).unwrap();

        #[cfg(debug_assertions)]
        //sign_message 0000000000000000000000000000000000000000000000000000000000000001
        assert_eq!(
            "0000000000000000000000000000000000000000000000000000000000000001",
            format!("{}", private_key.display_secret())
        );
        //println!(
        //    "118:{{\"private_key\": {:}}}",
        //    &private_key.display_secret()
        //);

        let key_pair = Keypair::from_secret_key(&secp, &private_key);
        let _pubkey_xo = XOnlyPublicKey::from_keypair(&key_pair);
        let (pubkey_xo, _parity) = key_pair.x_only_public_key();
        let pubkey_xot = pubkey_xo
            .add_tweak(&secp, &tweak)
            .expect("Improbable to fail with a randomly generated tweak");

        let public_xot_0_json = json!({
            "pubkey_xot_0": pubkey_xot.0.to_string()
        });
        //println!(
        //    "143:{{\"public_xot.0\": \"{:}\"}}",
        //    pubkey_xot.0.to_string()
        //);
        let public_xot_1_json = json!({
            "pubkey_xot_1": format!("{:?}", pubkey_xot.1)
        });
        //println!("144:{{\"public_xot.1\": \"{:?}\"}}", pubkey_xot.1);

        let (/*mut*/ x_public_key, _) = key_pair.x_only_public_key();
        let x_public_key_json = json!({
            "x_public_key": x_public_key.to_string()
        });
        //println!("141:{{\"x_public_key\": \"{:}\"}}", x_public_key);

        let x_original = x_public_key;
        let (tweaked, parity) = x_public_key
            .add_tweak(&secp, &tweak)
            .expect("Improbable to fail with a randomly generated tweak");
        assert!(x_original.tweak_add_check(&secp, &tweaked, parity, tweak));
        if env::args().len() == 2 {
            #[cfg(debug_assertions)]
            //println!("168:{{\"private_key\": {:}}}", &key_pair.display_secret());
            println!("169:{{\"public_key\": \"{}\"}}", &key_pair.public_key());
            process::exit(0);
        }

        use secp256k1::hashes::sha256;
        use secp256k1::Message;

        #[cfg(debug_assertions)]
        let empty_str: &'static str = "";
        #[cfg(debug_assertions)]
        //println!("empty_str={}", empty_str);
        #[cfg(debug_assertions)]
        let message_hash = Message::from_hashed_data::<sha256::Hash>(empty_str.as_bytes());
        #[cfg(debug_assertions)]
        assert_eq!(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            format!("{}", message_hash)
        );

        //sign_message 0000000000000000000000000000000000000000000000000000000000000005 ""
        let message_str = std::env::args().nth(2).expect("Missing message string");
        let message_str_json = json!({
            "message_str": message_str.to_string()
        });
        //println!("164:{{\"message_str\": \"{}\"}}", message_str);
        let message_hash = Message::from_hashed_data::<sha256::Hash>(message_str.as_bytes());

        //let message_hash_json = json!(

        //format!("{{\"message_hash\": \"{}\"}}", message_hash)

        //);

        let message_hash_json = json!({
            "message_hash": message_hash.to_string()
        });

        //println!("179:{{\"message_hash\": \"{}\"}}", message_hash);

        let sig = secp.sign_ecdsa(&message_hash, &private_key);
        assert!(secp
            .verify_ecdsa(&message_hash, &sig, &key_pair.public_key())
            .is_ok());

        let sig_json = json!({
            "sig": sig.to_string()
        });
        //// Define the data you want to store in the JSON object
        //let object0 = json!({
        //    "178_name": "John Doe",
        //    "179_age": 30,
        //    "180_city": "New York"
        //});
        //// Serialize the data into a JSON string
        //let json_string = serde_json::to_string(&object0).unwrap();

        //// Print the JSON string
        //println!("186:{}", json_string);

        //// Define the data for each object in the array
        //let object1 = json!({
        //    "190_name": "John Doe",
        //    "191_age": 30,
        //    "192_city": "New York"
        //});

        //let object2 = json!({
        //    "196_name": "Jane Doe",
        //    "197_age": 25,
        //    "198_city": "Los Angeles"
        //});

        // Create the JSON array
        //let mut json_array = Vec::new();
        //json_array.push(object0);
        //public_xot_0_json
        json_array.push(public_xot_0_json.clone());
        json_array.push(public_xot_1_json.clone());
        json_array.push(x_public_key_json.clone());
        json_array.push(message_str_json.clone());
        json_array.push(message_hash_json.clone());
        json_array.push(sig_json.clone());
        //json_array.push(object0.clone());
        //json_array.push(object1);
        //json_array.push(object2);

        // Convert the Vec to a JSON Value
        let json_value: Value = json!(json_array);

        // Print the JSON array
        println!("{}", json_value);

        //println!("{{\"sig\": \"{}\"}}", sig);
    } // end if env::args().len() > 1
    Ok(())
}
// This code defines a function to add two numbers
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

// This code is only compiled when running tests
#[cfg(test)]
mod tests {
    // Import the add function from the outer scope
    use super::*;

    // This function is marked as a test with the `#[test]` attribute
    #[test]
    fn test_add() {
        // This assertion checks if the sum of 1 and 2 is equal to 3
        assert_eq!(add(1, 2), 3);
    }
}
