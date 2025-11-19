use clap::Parser;
use std::path::PathBuf;
use std::fs;

use mig_rust::{SimpleLexer, SemanticAnalyzer, AnalyzedSubsystem};
use mig_rust::{PreprocessorConfig, PreprocessorFilter};
use mig_rust::parser::Parser as MigParser;
use mig_rust::codegen::c_user_stubs::CUserStubGenerator;
use mig_rust::codegen::c_server_stubs::CServerStubGenerator;
use mig_rust::codegen::c_header;
use mig_rust::codegen::rust_stubs::RustStubGenerator;

#[derive(Parser)]
#[command(name = "mig-rust")]
#[command(about = "Mach Interface Generator - Rust Implementation", long_about = None)]
struct Cli {
    /// Input .defs file(s)
    #[arg(required = true)]
    files: Vec<PathBuf>,

    /// Output directory
    #[arg(short, long, default_value = ".")]
    output: PathBuf,

    /// Generate user-side stubs
    #[arg(long)]
    user: bool,

    /// Generate server-side stubs
    #[arg(long)]
    server: bool,

    /// Generate header files
    #[arg(long)]
    header: bool,

    /// Generate Rust bindings
    #[arg(long)]
    rust: bool,

    /// Check syntax only (no generation)
    #[arg(long)]
    check: bool,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // If no output flags specified, generate everything
    let generate_all = !cli.user && !cli.server && !cli.header && !cli.rust && !cli.check;

    for file_path in &cli.files {
        if cli.verbose {
            println!("Processing: {}", file_path.display());
        }

        // Read input file
        let input = fs::read_to_string(file_path)?;

        // Lexical analysis
        if cli.verbose {
            println!("  Lexing...");
        }
        let mut lexer = SimpleLexer::new(input);
        let mut tokens = lexer.tokenize().map_err(|e| format!("Lexer error: {}", e))?;

        if cli.verbose {
            println!("  Tokenized {} tokens", tokens.len());
        }

        // Preprocessing (conditional compilation)
        if cli.verbose {
            println!("  Preprocessing...");
        }

        // Determine which preprocessor config to use
        let preproc_config = if cli.user && !cli.server {
            PreprocessorConfig::for_user()
        } else if cli.server && !cli.user {
            PreprocessorConfig::for_server()
        } else {
            // For check mode or when generating both, use default (all undefined)
            PreprocessorConfig::new()
        };

        let mut preproc_filter = PreprocessorFilter::new(preproc_config.symbols);
        tokens = preproc_filter.filter(tokens)
            .map_err(|e| format!("Preprocessor error: {}", e))?;

        if cli.verbose {
            println!("  After preprocessing: {} tokens", tokens.len());
        }

        // Parsing
        if cli.verbose {
            println!("  Parsing...");
        }
        let mut parser = MigParser::new(tokens);
        let subsystem = parser.parse()?;

        if cli.verbose {
            println!("  Parsed subsystem: {}", subsystem.name);
            println!("    Base: {}", subsystem.base);
            println!("    Statements: {}", subsystem.statements.len());
        }

        // Semantic analysis
        if cli.verbose {
            println!("  Analyzing...");
        }
        let mut analyzer = SemanticAnalyzer::new();
        let analyzed = analyzer.analyze(&subsystem)
            .map_err(|e| format!("Semantic error: {}", e))?;

        if cli.verbose {
            println!("    Routines: {}", analyzed.routines.len());
            println!("    Server prefix: {}", analyzed.server_prefix);
            println!("    User prefix: {}", analyzed.user_prefix);
        }

        // If check-only mode, stop here
        if cli.check {
            println!("✓ {} - Syntax OK", file_path.display());
            continue;
        }

        // Ensure output directory exists
        fs::create_dir_all(&cli.output)?;

        // Generate C code
        if cli.header || generate_all {
            generate_c_headers(&analyzed, &cli.output, cli.verbose)?;
        }

        if cli.user || generate_all {
            generate_c_user(&analyzed, &cli.output, cli.verbose)?;
        }

        if cli.server || generate_all {
            generate_c_server(&analyzed, &cli.output, cli.verbose)?;
        }

        if cli.rust {
            generate_rust_stubs(&analyzed, &cli.output, cli.verbose)?;
        }

        println!("✓ {} - Generated successfully", file_path.display());
    }

    Ok(())
}

fn generate_c_headers(
    analyzed: &AnalyzedSubsystem,
    output_dir: &PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("  Generating C headers...");
    }

    // Generate user header
    let user_header = c_header::generate_user_header(analyzed)?;
    let user_header_path = output_dir.join(format!("{}.h", analyzed.name));
    fs::write(&user_header_path, user_header)?;

    if verbose {
        println!("    → {}", user_header_path.display());
    }

    // Generate server header
    let server_header = c_header::generate_server_header(analyzed)?;
    let server_header_path = output_dir.join(format!("{}Server.h", analyzed.name));
    fs::write(&server_header_path, server_header)?;

    if verbose {
        println!("    → {}", server_header_path.display());
    }

    Ok(())
}

fn generate_c_user(
    analyzed: &AnalyzedSubsystem,
    output_dir: &PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("  Generating C user stubs...");
    }

    let generator = CUserStubGenerator::new();
    let user_impl = generator.generate(analyzed)?;
    let user_path = output_dir.join(format!("{}User.c", analyzed.name));
    fs::write(&user_path, user_impl)?;

    if verbose {
        println!("    → {}", user_path.display());
    }

    Ok(())
}

fn generate_c_server(
    analyzed: &AnalyzedSubsystem,
    output_dir: &PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("  Generating C server stubs...");
    }

    let generator = CServerStubGenerator::new();
    let server_impl = generator.generate(analyzed)?;
    let server_path = output_dir.join(format!("{}Server.c", analyzed.name));
    fs::write(&server_path, server_impl)?;

    if verbose {
        println!("    → {}", server_path.display());
    }

    Ok(())
}

fn generate_rust_stubs(
    analyzed: &AnalyzedSubsystem,
    output_dir: &PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("  Generating Rust stubs...");
    }

    let generator = RustStubGenerator::new()
        .with_async()         // Generate async API
        .with_server_traits(); // Generate server traits

    let rust_impl = generator.generate(analyzed)?;
    let rust_path = output_dir.join(format!("{}.rs", analyzed.name));
    fs::write(&rust_path, rust_impl)?;

    if verbose {
        println!("    → {} (type-safe Rust IPC)", rust_path.display());
    }

    Ok(())
}
