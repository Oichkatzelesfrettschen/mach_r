use clap::Parser;
use std::path::PathBuf;
use std::fs;

use mig_rust::{SimpleLexer, Subsystem, SemanticAnalyzer, AnalyzedSubsystem};
use mig_rust::parser::Parser as MigParser;
use mig_rust::codegen::c_generator::CCodeGenerator;
use mig_rust::codegen::c_user_stubs::CUserStubGenerator;
use mig_rust::codegen::CodeGenerator;

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
        let tokens = lexer.tokenize().map_err(|e| format!("Lexer error: {}", e))?;

        if cli.verbose {
            println!("  Tokenized {} tokens", tokens.len());
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
        let c_gen = CCodeGenerator::new();

        if cli.header || generate_all {
            generate_c_header(&subsystem, &c_gen, &cli.output, cli.verbose)?;
        }

        if cli.user || generate_all {
            generate_c_user(&analyzed, &cli.output, cli.verbose)?;
        }

        if cli.server || generate_all {
            generate_c_server(&subsystem, &c_gen, &cli.output, cli.verbose)?;
        }

        if cli.rust {
            println!("Warning: Rust code generation not yet implemented");
        }

        println!("✓ {} - Generated successfully", file_path.display());
    }

    Ok(())
}

fn generate_c_header(
    subsystem: &Subsystem,
    generator: &CCodeGenerator,
    output_dir: &PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("  Generating C header...");
    }

    let header = generator.generate_user_header(subsystem)?;
    let header_path = output_dir.join(format!("{}.h", subsystem.name));
    fs::write(&header_path, header)?;

    if verbose {
        println!("    → {}", header_path.display());
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
    subsystem: &Subsystem,
    generator: &CCodeGenerator,
    output_dir: &PathBuf,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if verbose {
        println!("  Generating C server stubs...");
    }

    let server_header = generator.generate_server_header(subsystem)?;
    let server_header_path = output_dir.join(format!("{}Server.h", subsystem.name));
    fs::write(&server_header_path, server_header)?;

    let server_impl = generator.generate_server_impl(subsystem)?;
    let server_path = output_dir.join(format!("{}Server.c", subsystem.name));
    fs::write(&server_path, server_impl)?;

    if verbose {
        println!("    → {}", server_header_path.display());
        println!("    → {}", server_path.display());
    }

    Ok(())
}
