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

    match args.first().map(|s| s.as_str()) {
        // hemera tree <file>  — display full tree structure
        Some("tree") => {
            let rest: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
            if rest.len() != 1 {
                eprintln!("hemera: tree requires <file>");
                process::exit(1);
            }
            process::exit(show_tree(rest[0]));
        }
        // hemera prove <file> [chunk | start:end]  — generate inclusion proof
        Some("prove") => {
            let rest: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
            match rest.len() {
                1 => process::exit(prove_node(rest[0], 0, 1)),
                2 => {
                    if let Some((s, e)) = rest[1].split_once(':') {
                        let start: u64 = s.parse().unwrap_or_else(|_| {
                            eprintln!("hemera: invalid range start: {s}");
                            process::exit(1);
                        });
                        let end: u64 = e.parse().unwrap_or_else(|_| {
                            eprintln!("hemera: invalid range end: {e}");
                            process::exit(1);
                        });
                        process::exit(prove_node(rest[0], start, end));
                    } else {
                        let idx: u64 = rest[1].parse().unwrap_or_else(|_| {
                            eprintln!("hemera: invalid chunk index: {}", rest[1]);
                            process::exit(1);
                        });
                        process::exit(prove_node(rest[0], idx, idx + 1));
                    }
                }
                _ => {
                    eprintln!("hemera: prove requires <file> [chunk | start:end]");
                    process::exit(1);
                }
            }
        }
        // hemera verify <file> <hash>  — verify single file against hash
        // hemera verify <checksums>    — verify batch from checksum file
        Some("verify") => {
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
        _ => {}
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

fn show_tree(path: &str) -> i32 {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    let n = cyber_hemera::tree::num_chunks(data.len());
    let tree = cyber_hemera::tree::build_tree(&data);

    println!("file: {path}");
    println!("size: {} bytes", data.len());
    println!("chunks: {n}");
    println!("depth: {}", tree.depth);
    println!("root: {}", tree.hash);
    println!();
    print_tree_node(&tree, "", "");
    0
}

fn print_tree_node(node: &cyber_hemera::tree::TreeNode, connector: &str, prefix: &str) {
    let short_hash = &node.hash.to_string()[..16];
    let idx = node.index;
    if let Some(chunk) = node.chunk_index {
        println!("{connector}[{idx}] chunk({chunk}) {short_hash}…");
    } else {
        println!("{connector}[{idx}] node {short_hash}…");
    }

    if let (Some(left), Some(right)) = (&node.left, &node.right) {
        print_tree_node(left, &format!("{prefix}├── "), &format!("{prefix}│   "));
        print_tree_node(right, &format!("{prefix}└── "), &format!("{prefix}    "));
    }
}

fn prove_node(path: &str, start: u64, end: u64) -> i32 {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    let n = cyber_hemera::tree::num_chunks(data.len());
    if start >= end || end > n {
        eprintln!("hemera: invalid range [{start}..{end}) for {n} chunks");
        return 1;
    }

    let (root, proof) = cyber_hemera::tree::prove_range(&data, start, end);

    println!("root: {root}");
    if end - start == 1 {
        println!("chunk: {start}/{n}");
    } else {
        println!("range: [{start}..{end}) of {n} chunks");
    }
    println!("depth: {}", proof.siblings.len());
    for (i, sibling) in proof.siblings.iter().enumerate() {
        let (dir, hash) = match sibling {
            cyber_hemera::tree::Sibling::Left(h) => ("L", h),
            cyber_hemera::tree::Sibling::Right(h) => ("R", h),
        };
        println!("  [{i}] {dir} {hash}");
    }
    0
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
  hemera tree file.txt             Show tree structure
  hemera prove file.txt [chunk]    Leaf inclusion proof
  hemera prove file.txt 0:4        Subtree inclusion proof
  hemera verify file.txt <hash>    Verify file against hash
  hemera verify sums.txt           Verify checksums from file

  -h, --help  Print this help"
    );
}
