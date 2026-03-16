use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;
use std::time::Instant;

use cyber_hemera_wgsl::GpuContext;

fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Print a transient status line to stderr (overwritten by next status or final output).
fn status(msg: &str) {
    if io::IsTerminal::is_terminal(&io::stderr()) {
        eprint!("\x1b[2K\r\x1b[90m{msg}\x1b[0m");
    }
}

/// Clear the status line.
fn status_clear() {
    if io::IsTerminal::is_terminal(&io::stderr()) {
        eprint!("\x1b[2K\r");
    }
}

/// Progress callback that shows percentage on the status line (0.1% resolution).
fn progress_status(label: &str) -> impl Fn(usize, usize) + '_ {
    let last_permille = std::cell::Cell::new(u16::MAX);
    move |done, total| {
        if total == 0 { return; }
        let permille = ((done as u64 * 1000) / total as u64) as u16;
        if permille != last_permille.get() {
            last_permille.set(permille);
            status(&format!("{label} {}.{}%", permille / 10, permille % 10));
        }
    }
}

// ── backend selection ─────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Backend {
    Cpu,
    Gpu,
}

impl std::fmt::Display for Backend {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Backend::Cpu => write!(f, "cpu"),
            Backend::Gpu => write!(f, "gpu"),
        }
    }
}

struct Ctx {
    gpu: Option<GpuContext>,
    forced: Option<Backend>,
}

impl Ctx {
    fn new(forced: Option<Backend>) -> Self {
        // Default to CPU — WGSL u64 emulation is slower than native CPU
        // on Apple Silicon. GPU available via --gpu for testing.
        let gpu = if forced == Some(Backend::Gpu) {
            let g = pollster::block_on(GpuContext::new());
            if g.is_none() {
                eprintln!("hemera: --gpu requested but no GPU adapter available");
                process::exit(1);
            }
            g
        } else {
            None
        };
        Self { gpu, forced }
    }

    fn backend(&self) -> Backend {
        if self.forced == Some(Backend::Cpu) {
            return Backend::Cpu;
        }
        if self.gpu.is_some() {
            Backend::Gpu
        } else {
            Backend::Cpu
        }
    }

    fn gpu(&self) -> &GpuContext {
        self.gpu.as_ref().unwrap()
    }

    fn root_hash(&self, data: &[u8], label: &str) -> (cyber_hemera::Hash, Backend) {
        let b = self.backend();
        let cb = progress_status(label);
        let h = if b == Backend::Gpu {
            pollster::block_on(self.gpu().root_hash_with_progress(data, &cb))
        } else {
            cyber_hemera::tree::root_hash_with_progress(data, &cb)
        };
        (h, b)
    }

    fn outboard(&self, data: &[u8]) -> ((cyber_hemera::Hash, Vec<u8>), Backend) {
        let b = self.backend();
        let result = if b == Backend::Gpu {
            pollster::block_on(self.gpu().outboard(data))
        } else {
            cyber_hemera::stream::outboard(data)
        };
        (result, b)
    }
}

/// Strip --gpu / --cpu from args, return (forced backend, remaining args).
fn parse_backend_flag(args: &[String]) -> (Option<Backend>, Vec<String>) {
    let mut forced = None;
    let mut rest = Vec::new();
    for a in args {
        match a.as_str() {
            "--gpu" => forced = Some(Backend::Gpu),
            "--cpu" => forced = Some(Backend::Cpu),
            _ => rest.push(a.clone()),
        }
    }
    (forced, rest)
}

fn print_timing(backend: Backend, elapsed: std::time::Duration) {
    let us = elapsed.as_nanos() as f64 / 1000.0;
    if us < 1000.0 {
        eprint!("\x1b[90m[{backend} {us:.0}us]\x1b[0m ");
    } else if us < 1_000_000.0 {
        eprint!("\x1b[90m[{backend} {:.2}ms]\x1b[0m ", us / 1000.0);
    } else {
        eprint!("\x1b[90m[{backend} {:.2}s]\x1b[0m ", us / 1_000_000.0);
    }
}

#[allow(unknown_lints, rs_no_vec, rs_no_string)]
fn main() {
    let all_args: Vec<String> = std::env::args().skip(1).collect();
    let (forced, args) = parse_backend_flag(&all_args);

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return;
    }

    match args.first().map(|s| s.as_str()) {
        Some("tree") => {
            if args.len() != 2 {
                eprintln!("hemera: tree requires <file>");
                process::exit(1);
            }
            process::exit(show_tree(&args[1]));
        }
        Some("prove") => {
            match args.len() {
                2 => process::exit(prove_node(&args[1], 0, 1)),
                3 => {
                    if let Some((s, e)) = args[2].split_once(':') {
                        let start: u64 = s.parse().unwrap_or_else(|_| {
                            eprintln!("hemera: invalid range start: {s}");
                            process::exit(1);
                        });
                        let end: u64 = e.parse().unwrap_or_else(|_| {
                            eprintln!("hemera: invalid range end: {e}");
                            process::exit(1);
                        });
                        process::exit(prove_node(&args[1], start, end));
                    } else {
                        let idx: u64 = args[2].parse().unwrap_or_else(|_| {
                            eprintln!("hemera: invalid chunk index: {}", args[2]);
                            process::exit(1);
                        });
                        process::exit(prove_node(&args[1], idx, idx + 1));
                    }
                }
                _ => {
                    eprintln!("hemera: prove requires <file> [chunk | start:end]");
                    process::exit(1);
                }
            }
        }
        Some("encode") => {
            let ctx = Ctx::new(forced);
            match args.len() {
                2 => process::exit(cmd_encode(&ctx, &args[1], None)),
                4 if args[2] == "-o" => process::exit(cmd_encode(&ctx, &args[1], Some(&args[3]))),
                _ => {
                    eprintln!("hemera: encode requires <file> [-o output]");
                    process::exit(1);
                }
            }
        }
        Some("decode") => match args.len() {
            3 => process::exit(cmd_decode(&args[1], &args[2], None)),
            5 if args[3] == "-o" => {
                process::exit(cmd_decode(&args[1], &args[2], Some(&args[4])))
            }
            _ => {
                eprintln!("hemera: decode requires <file> <hash> [-o output]");
                process::exit(1);
            }
        },
        Some("outboard") => {
            let ctx = Ctx::new(forced);
            match args.len() {
                2 => process::exit(cmd_outboard(&ctx, &args[1], None)),
                4 if args[2] == "-o" => process::exit(cmd_outboard(&ctx, &args[1], Some(&args[3]))),
                _ => {
                    eprintln!("hemera: outboard requires <file> [-o output]");
                    process::exit(1);
                }
            }
        }
        Some("keyed-hash") => {
            if args.len() != 3 {
                eprintln!("hemera: keyed-hash requires <key-hex> <file>");
                process::exit(1);
            }
            let ctx = Ctx::new(forced);
            process::exit(cmd_keyed_hash(&ctx, &args[1], &args[2]));
        }
        Some("derive-key") => {
            if args.len() != 3 {
                eprintln!("hemera: derive-key requires <context> <file>");
                process::exit(1);
            }
            let ctx = Ctx::new(forced);
            process::exit(cmd_derive_key(&ctx, &args[1], &args[2]));
        }
        Some("prove-batch") => {
            if args.len() < 3 {
                eprintln!("hemera: prove-batch requires <file> <index>...");
                process::exit(1);
            }
            let indices: Vec<u64> = args[2..].iter().map(|s| {
                s.parse().unwrap_or_else(|_| {
                    eprintln!("hemera: invalid chunk index: {s}");
                    process::exit(1);
                })
            }).collect();
            process::exit(cmd_prove_batch(&args[1], &indices));
        }
        Some("verify-batch") => {
            if args.len() < 3 {
                eprintln!("hemera: verify-batch requires <file> <root-hash>");
                process::exit(1);
            }
            process::exit(cmd_verify_batch(&args[1], &args[2]));
        }
        Some("sparse") => {
            if args.len() < 2 {
                eprintln!("hemera: sparse requires a subcommand (new, insert, get, prove, verify, root)");
                process::exit(1);
            }
            process::exit(cmd_sparse(&args[1..]));
        }
        Some("verify") => match args.len() {
            3 => {
                let ctx = Ctx::new(forced);
                process::exit(verify_single(&ctx, &args[1], &args[2]));
            }
            2 => {
                let ctx = Ctx::new(forced);
                process::exit(verify_checksums(&ctx, &args[1]));
            }
            _ => {
                eprintln!("hemera: verify requires <file> <hash> or <checksums-file>");
                process::exit(1);
            }
        },
        _ => {}
    }

    let ctx = Ctx::new(forced);
    let has_files = args.iter().any(|a| !a.starts_with('-'));

    if !has_files {
        if io::IsTerminal::is_terminal(&io::stdin()) {
            print_usage();
            return;
        }
        status("reading stdin…");
        let mut data = Vec::new();
        io::stdin().read_to_end(&mut data).unwrap_or_else(|e| {
            eprintln!("hemera: {e}");
            process::exit(1);
        });
        let t = Instant::now();
        let (hash, backend) = ctx.root_hash(&data, "hashing");
        status_clear();
        print_timing(backend, t.elapsed());
        println!("{}  -", hash);
    } else {
        for arg in &args {
            if !arg.starts_with('-') {
                hash_path(&ctx, Path::new(arg));
            }
        }
    }
}

#[allow(unknown_lints, rs_no_vec)]
fn hash_path(ctx: &Ctx, path: &Path) {
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
            hash_path(ctx, &entry.path());
        }
    } else {
        match hash_file(ctx, path) {
            Ok((hex, backend, elapsed)) => {
                print_timing(backend, elapsed);
                println!("{hex}  {}", path.display());
            }
            Err(e) => {
                eprintln!("hemera: {}: {e}", path.display());
                process::exit(1);
            }
        }
    }
}

#[allow(unknown_lints, rs_no_vec)]
fn hash_file(ctx: &Ctx, path: &Path) -> io::Result<(String, Backend, std::time::Duration)> {
    let meta = fs::metadata(path)?;
    status(&format!("reading {} ({})", path.display(), fmt_size(meta.len())));
    let mut file = File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    let label = format!("hashing {}", path.display());
    let t = Instant::now();
    let (hash, backend) = ctx.root_hash(&data, &label);
    status_clear();
    Ok((hash.to_string(), backend, t.elapsed()))
}

fn show_tree(path: &str) -> i32 {
    let size = match fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };
    status(&format!("reading {path} ({})…", fmt_size(size)));
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    let n = cyber_hemera::tree::num_chunks(data.len());
    let tree = cyber_hemera::tree::build_tree_with_progress(&data, progress_status("building tree"));
    status_clear();

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
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    status(&format!("reading {path} ({})…", fmt_size(size)));
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

    status(&format!("proving range [{start}..{end})…"));
    let (root, proof) = cyber_hemera::tree::prove_range(&data, start, end);
    status_clear();

    println!("root: {root}");
    if end - start == 1 {
        println!("chunk: {start}/{n}");
    } else {
        println!("range: [{start}..{end}) of {n} chunks");
    }
    println!("depth: {}", proof.depth());
    for (i, sibling) in proof.siblings().iter().enumerate() {
        let (dir, hash) = match sibling {
            cyber_hemera::tree::Sibling::Left(h) => ("L", h),
            cyber_hemera::tree::Sibling::Right(h) => ("R", h),
        };
        println!("  [{i}] {dir} {hash}");
    }
    0
}

fn verify_single(ctx: &Ctx, path: &str, expected: &str) -> i32 {
    match hash_file(ctx, Path::new(path)) {
        Ok((actual, backend, elapsed)) => {
            print_timing(backend, elapsed);
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

fn verify_checksums(ctx: &Ctx, path: &str) -> i32 {
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
        match hash_file(ctx, Path::new(filename.trim())) {
            Ok((actual_hex, backend, elapsed)) => {
                print_timing(backend, elapsed);
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

fn cmd_encode(_ctx: &Ctx, path: &str, output: Option<&str>) -> i32 {
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    status(&format!("reading {path} ({})…", fmt_size(size)));
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    status(&format!("encoding ({})…", fmt_size(size)));
    let t = Instant::now();
    let (root, encoded) = cyber_hemera::stream::encode(&data);
    status_clear();
    let elapsed = t.elapsed();
    let default_path = format!("{path}.hemera");
    let out_path = output.unwrap_or(&default_path);

    if let Err(e) = fs::write(out_path, &encoded) {
        eprintln!("hemera: {out_path}: {e}");
        return 1;
    }

    // encode uses CPU internally (stream format needs sequential structure),
    // but we report the context backend for consistency
    print_timing(Backend::Cpu, elapsed);
    println!("{root}  {out_path}");
    0
}

fn cmd_decode(path: &str, hash_hex: &str, output: Option<&str>) -> i32 {
    let encoded = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    let root = match parse_hash(hash_hex) {
        Some(h) => h,
        None => {
            eprintln!("hemera: invalid hash: {hash_hex}");
            return 1;
        }
    };

    match cyber_hemera::stream::decode(&encoded, &root) {
        Ok(data) => {
            if let Some(out_path) = output {
                if let Err(e) = fs::write(out_path, &data) {
                    eprintln!("hemera: {out_path}: {e}");
                    return 1;
                }
                println!("{path}: OK → {out_path}");
            } else {
                let stdout = io::stdout();
                let mut handle = stdout.lock();
                if let Err(e) = handle.write_all(&data) {
                    eprintln!("hemera: {e}");
                    return 1;
                }
            }
            0
        }
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            1
        }
    }
}

fn cmd_outboard(ctx: &Ctx, path: &str, output: Option<&str>) -> i32 {
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    status(&format!("reading {path} ({})…", fmt_size(size)));
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    status(&format!("computing outboard ({})…", fmt_size(size)));
    let t = Instant::now();
    let ((root, ob), backend) = ctx.outboard(&data);
    status_clear();
    let elapsed = t.elapsed();
    let default_path = format!("{path}.obao");
    let out_path = output.unwrap_or(&default_path);

    if let Err(e) = fs::write(out_path, &ob) {
        eprintln!("hemera: {out_path}: {e}");
        return 1;
    }

    print_timing(backend, elapsed);
    println!("{root}  {out_path}");
    0
}

fn cmd_keyed_hash(_ctx: &Ctx, key_hex: &str, path: &str) -> i32 {
    if key_hex.len() != cyber_hemera::OUTPUT_BYTES * 2 {
        eprintln!(
            "hemera: key must be {} hex chars ({} bytes)",
            cyber_hemera::OUTPUT_BYTES * 2,
            cyber_hemera::OUTPUT_BYTES
        );
        return 1;
    }

    let key = match parse_hex_fixed::<{ cyber_hemera::OUTPUT_BYTES }>(key_hex) {
        Some(k) => k,
        None => {
            eprintln!("hemera: invalid hex key: {key_hex}");
            return 1;
        }
    };

    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    // keyed_hash is a single sponge — CPU path
    let t = Instant::now();
    let h = cyber_hemera::keyed_hash(&key, &data);
    let elapsed = t.elapsed();

    print_timing(Backend::Cpu, elapsed);
    println!("{h}  {path}");
    0
}

fn cmd_derive_key(_ctx: &Ctx, context: &str, path: &str) -> i32 {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    let t = Instant::now();
    let key = cyber_hemera::derive_key(context, &data);
    let elapsed = t.elapsed();

    print_timing(Backend::Cpu, elapsed);
    for byte in &key {
        print!("{byte:02x}");
    }
    println!("  {path}");
    0
}

fn cmd_prove_batch(path: &str, indices: &[u64]) -> i32 {
    let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    status(&format!("reading {path} ({})…", fmt_size(size)));
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("hemera: {path}: {e}");
            return 1;
        }
    };

    let mut sorted = indices.to_vec();
    sorted.sort();
    sorted.dedup();

    status(&format!("proving batch ({} indices)…", sorted.len()));
    let (root, proof) = cyber_hemera::batch::prove_batch(&data, &sorted);
    status_clear();
    println!("root: {root}");
    println!("indices: {sorted:?}");
    println!("siblings: {}", proof.siblings.len());
    println!("chunks: {}", proof.num_chunks);
    for (i, sib) in proof.siblings.iter().enumerate() {
        println!("  [{i}] {sib}");
    }
    0
}

fn cmd_verify_batch(proof_path: &str, root_hex: &str) -> i32 {
    let content = match fs::read_to_string(proof_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("hemera: {proof_path}: {e}");
            return 1;
        }
    };

    let root = match parse_hash(root_hex) {
        Some(h) => h,
        None => {
            eprintln!("hemera: invalid root hash: {root_hex}");
            return 1;
        }
    };

    eprintln!("hemera: verify-batch expects structured proof input (not yet implemented)");
    eprintln!("  use the library API: cyber_hemera::batch::verify_batch()");
    let _ = (content, root);
    1
}

#[allow(unknown_lints, rs_no_vec, rs_no_string)]
fn cmd_sparse(args: &[String]) -> i32 {
    match args[0].as_str() {
        "hash-leaf" => {
            if args.len() != 3 {
                eprintln!("hemera: sparse hash-leaf requires <key-hex-32bytes> <file>");
                return 1;
            }
            let key = match parse_hex_fixed::<32>(&args[1]) {
                Some(k) => k,
                None => {
                    eprintln!("hemera: invalid 32-byte hex key: {}", args[1]);
                    return 1;
                }
            };
            let data = match fs::read(&args[2]) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("hemera: {}: {e}", args[2]);
                    return 1;
                }
            };
            let mut input = Vec::with_capacity(32 + data.len());
            input.extend_from_slice(&key);
            input.extend_from_slice(&data);
            let h = cyber_hemera::tree::hash_leaf(&input, 0, false);
            println!("{h}");
            0
        }
        "prove" => {
            eprintln!("hemera: sparse prove — use the library API: SparseTree::prove()");
            eprintln!("  interactive sparse tree operations require state persistence");
            eprintln!("  which is beyond the scope of a stateless CLI");
            1
        }
        _ => {
            eprintln!("hemera: unknown sparse subcommand: {}", args[0]);
            eprintln!("  available: hash-leaf, prove");
            1
        }
    }
}

fn parse_hash(hex: &str) -> Option<cyber_hemera::Hash> {
    let bytes = parse_hex_fixed::<{ cyber_hemera::OUTPUT_BYTES }>(hex)?;
    Some(cyber_hemera::Hash::from_bytes(bytes))
}

fn parse_hex_fixed<const N: usize>(hex: &str) -> Option<[u8; N]> {
    if hex.len() != N * 2 {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
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
  hemera file1.txt file2.txt         Hash files
  hemera src/                        Hash directory (recursive)
  echo hello | hemera                Hash stdin
  hemera tree file.txt               Show tree structure
  hemera prove file.txt [chunk]      Leaf inclusion proof
  hemera prove file.txt 0:4          Subtree inclusion proof
  hemera verify file.txt <hash>      Verify file against hash
  hemera verify sums.txt             Verify checksums from file
  hemera encode file.txt [-o out]    Encode to verified stream
  hemera decode file.hemera <hash>   Decode and verify stream
  hemera outboard file.txt [-o out]  Compute outboard hash tree
  hemera prove-batch file 0 1 3      Batch inclusion proof
  hemera sparse hash-leaf <key> file Sparse leaf hash
  hemera keyed-hash <key-hex> file   Keyed hash
  hemera derive-key <context> file   Derive key from context
\x1b[90m
  flags:    --gpu  force GPU backend
            --cpu  force CPU backend
            (default: CPU)
\x1b[0m
  -h, --help  Print this help"
    );
}
