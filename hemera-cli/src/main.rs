use std::fs::{self, File};
use std::io::{self, Read};
use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return;
    }

    // hemera verify <file> <hash>  — verify single file against hash
    // hemera verify <checksums>    — verify batch from checksum file
    if args.first().map(|s| s.as_str()) == Some("verify") {
        let rest: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
        match rest.len() {
            2 => process::exit(verify_single(rest[0], rest[1])),
            1 => process::exit(verify_checksums(rest[0])),
            _ => {
                eprintln!("hemera: verify requires <file> <hash> or <checksums-file>");
                process::exit(1);
            }
        }
    }

    let files: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .map(|s| s.as_str())
        .collect();

    if files.is_empty() {
        if io::IsTerminal::is_terminal(&io::stdin()) {
            print_usage();
            return;
        }
        let mut data = Vec::new();
        io::stdin().read_to_end(&mut data).unwrap_or_else(|e| {
            eprintln!("hemera: {e}");
            process::exit(1);
        });
        let hex = cyber_hemera::tree::root_hash(&data).to_string();
        println!("{hex}  -");
    } else {
        for path in &files {
            hash_path(Path::new(path));
        }
    }
}

fn hash_path(path: &Path) {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("hemera: {}: {e}", path.display());
            process::exit(1);
        }
    };

    if meta.is_dir() {
        let mut entries: Vec<_> = match fs::read_dir(path) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(e) => {
                eprintln!("hemera: {}: {e}", path.display());
                process::exit(1);
            }
        };
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            hash_path(&entry.path());
        }
    } else {
        match hash_file(path) {
            Ok(hex) => println!("{hex}  {}", path.display()),
            Err(e) => {
                eprintln!("hemera: {}: {e}", path.display());
                process::exit(1);
            }
        }
    }
}

fn hash_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    Ok(cyber_hemera::tree::root_hash(&data).to_string())
}

fn verify_single(path: &str, expected: &str) -> i32 {
    match hash_file(Path::new(path)) {
        Ok(actual) => {
            if actual == expected {
                println!("{path}: OK");
                0
            } else {
                println!("{path}: FAILED");
                1
            }
        }
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            1
        }
    }
}

fn verify_checksums(path: &str) -> i32 {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    let mut failures = 0;
    let mut total = 0;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let Some((expected_hex, filename)) = line.split_once("  ") else {
            eprintln!("hemera: malformed line: {line}");
            failures += 1;
            continue;
        };

        total += 1;
        match hash_file(Path::new(filename.trim())) {
            Ok(actual_hex) => {
                if actual_hex == expected_hex.trim() {
                    println!("{filename}: OK");
                } else {
                    println!("{filename}: FAILED");
                    failures += 1;
                }
            }
            Err(e) => {
                eprintln!("hemera: {filename}: {e}");
                failures += 1;
            }
        }
    }

    if failures > 0 {
        eprintln!("hemera: WARNING: {failures} of {total} computed checksums did NOT match");
        1
    } else {
        0
    }
}

fn print_usage() {
    eprintln!(
        "\
\x1b[31m
    ██╗  ██╗███████╗███╗   ███╗███████╗██████╗  █████╗
\x1b[33m    ██║  ██║██╔════╝████╗ ████║██╔════╝██╔══██╗██╔══██╗
\x1b[32m    ███████║█████╗  ██╔████╔██║█████╗  ██████╔╝███████║
\x1b[36m    ██╔══██║██╔══╝  ██║╚██╔╝██║██╔══╝  ██╔══██╗██╔══██║
\x1b[34m    ██║  ██║███████╗██║ ╚═╝ ██║███████╗██║  ██║██║  ██║
\x1b[35m    ╚═╝  ╚═╝╚══════╝╚═╝     ╚═╝╚══════╝╚═╝  ╚═╝╚═╝  ╚═╝
\x1b[0m\x1b[37m    the hash for superintelligence\x1b[0m
\x1b[90m
    Poseidon2 · Goldilocks field · p = 2^64 - 2^32 + 1
    t=16  R_F=8  R_P=64  d=7  rate=8  output=64B
    genesis: [0x63, 0x79, 0x62, 0x65, 0x72]
\x1b[0m
  hemera file1.txt file2.txt       Hash files
  hemera src/                      Hash directory (recursive)
  echo hello | hemera              Hash stdin
  hemera verify file.txt <hash>    Verify file against hash
  hemera verify sums.txt           Verify checksums from file

  -h, --help  Print this help"
    );
}
