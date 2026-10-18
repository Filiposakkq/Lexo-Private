//! `lexo-pro-licence`: the tool of the author for licences (private). The server will make the same files with the same functions; until it exists a licence can be issued by hand.
//!
//! ```text
//! lexo-pro-licence keygen <folder>                      writes licence-private.key (keep it secret!) and prints the public key (put it into PUBLIC_KEY_HEX)
//! lexo-pro-licence issue --key <private.key> --id <id> [--features all|a,b] [--major 1] [--device any|<id>] [--days 30] [--out licence.json]
//! lexo-pro-licence show <licence.json> <public key hex>  checks a file
//! lexo-pro-licence device                               prints the device id of this computer
//! ```
use ed25519_dalek::SigningKey;
use lexo_pro::licence;
use serde_json::json;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => {
            let folder = std::path::PathBuf::from(args.get(1).map_or(".", String::as_str));
            let mut seed = [0u8; 32];
            getrandom::getrandom(&mut seed).unwrap_or_else(|e| fail(&format!("no random numbers: {e}")));
            let key = SigningKey::from_bytes(&seed);
            let path = folder.join("licence-private.key");
            if path.exists() {
                fail("licence-private.key exists already; it is not overwritten.");
            }
            std::fs::create_dir_all(&folder).unwrap_or_else(|e| fail(&e.to_string()));
            std::fs::write(&path, hex(&seed)).unwrap_or_else(|e| fail(&e.to_string()));
            println!("private key: {} (never commit it, never put it into a build)", path.display());
            println!("public key:  {}", hex(key.verifying_key().as_bytes()));
        }
        Some("issue") => {
            let key_path = flag(&args, "--key").unwrap_or_else(|| fail("--key is required"));
            let text = std::fs::read_to_string(&key_path).unwrap_or_else(|e| fail(&format!("{key_path}: {e}")));
            let seed: [u8; 32] = (0..64)
                .step_by(2)
                .map(|i| u8::from_str_radix(text.trim().get(i..i + 2).unwrap_or("zz"), 16).unwrap_or_else(|_| fail("the key file is not a key")))
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap_or_else(|_| fail("the key file is not a key"));
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
            let features: Vec<String> = flag(&args, "--features").unwrap_or_else(|| "all".into()).split(',').map(|s| s.trim().to_string()).collect();
            let payload = json!({
                "v": 1,
                "id": flag(&args, "--id").unwrap_or_else(|| fail("--id is required")),
                "features": features,
                "major": flag(&args, "--major").and_then(|s| s.parse::<u64>().ok()).unwrap_or(licence::PRODUCT_MAJOR),
                "device": flag(&args, "--device").unwrap_or_else(|| "any".into()),
                "issued": now,
                "check_days": flag(&args, "--days").and_then(|s| s.parse::<i64>().ok()).unwrap_or(14),
            });
            let file = licence::sign(&SigningKey::from_bytes(&seed), &payload);
            let out = flag(&args, "--out").unwrap_or_else(|| "licence.json".into());
            std::fs::write(&out, file.to_string()).unwrap_or_else(|e| fail(&format!("{out}: {e}")));
            println!("written {out}");
        }
        Some("show") => {
            let (Some(path), Some(key)) = (args.get(1), args.get(2)) else { fail("usage: show <licence.json> <public key hex>") };
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{path}: {e}")));
            let bytes: [u8; 32] = (0..64)
                .step_by(2)
                .map(|i| u8::from_str_radix(key.get(i..i + 2).unwrap_or("zz"), 16).unwrap_or_else(|_| fail("the public key is 64 hex digits")))
                .collect::<Vec<u8>>()
                .try_into()
                .unwrap_or_else(|_| fail("the public key is 64 hex digits"));
            let verifying = ed25519_dalek::VerifyingKey::from_bytes(&bytes).unwrap_or_else(|_| fail("the public key is not a key"));
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
            println!("{}", licence::evaluate(&text, now, &licence::device_id(), Some(&verifying)).to_json());
        }
        Some("device") => println!("{}", licence::device_id()),
        _ => fail("usage: lexo-pro-licence keygen|issue|show|device (see the top of src/licence_tool.rs)"),
    }
}
