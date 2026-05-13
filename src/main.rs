use anyhow::Result;
use clap::Parser;
use colored::*;
use djlintr::{config::Config, format, lint, linter::LintError};
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Files or directories to process
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Reformat files
    #[arg(short, long)]
    reformat: bool,

    /// Lint files
    #[arg(short, long)]
    lint: bool,

    /// File extensions to include
    #[arg(short, long, default_value = "html")]
    extension: Vec<String>,

    /// Number of threads to use
    #[arg(short, long)]
    threads: Option<usize>,

    /// Return non-zero exit code if issues found
    #[arg(long)]
    check: bool,

    /// Maximum length for attributes before wrapping
    #[arg(long, default_value = "70")]
    max_attribute_length: usize,

    /// Comma-separated list of custom block tags
    #[arg(long, value_delimiter = ',')]
    custom_blocks: Option<Vec<String>>,

    /// Profile for the template language
    #[arg(short, long)]
    profile: Option<String>,

    /// Consolidate blank lines down to x lines
    #[arg(long, default_value = "1")]
    max_blank_lines: usize,
}

struct FileResult {
    path: PathBuf,
    errors: Vec<LintError>,
    reformatted: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let mut config = Config::load();

    config.max_attribute_length = args.max_attribute_length;

    if let Some(custom_blocks) = &args.custom_blocks {
        config.custom_blocks.extend(custom_blocks.clone());
    }

    if let Some(profile) = &args.profile {
        config.profile = profile.clone();
    }

    config.max_blank_lines = args.max_blank_lines;

    if let Some(threads) = args.threads {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build_global()?;
    }

    let files = collect_files(&args.paths, &args.extension);
    if files.is_empty() {
        println!("No files found to process.");
        return Ok(());
    }

    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:>12.cyan.bold} [{bar:40.cyan/blue}] {pos}/{len} {msg} {elapsed}")?
            .progress_chars("━> "),
    );
    pb.set_prefix("Linting");

    let results: Vec<Result<FileResult>> = files
        .par_iter()
        .progress_with(pb.clone())
        .map(|path| process_file(path, &args, &config))
        .collect();

    pb.finish_and_clear();

    let mut total_errors = 0;
    let mut total_reformatted = 0;

    for result in results {
        match result {
            Ok(res) => {
                if !res.errors.is_empty() || res.reformatted {
                    total_errors += res.errors.len();
                    if res.reformatted {
                        total_reformatted += 1;
                    }

                    println!("\n{}", res.path.to_string_lossy().bold());
                    println!("{}", "─".repeat(res.path.to_string_lossy().len()).dimmed());

                    for error in res.errors {
                        println!(
                            "{} {:>3}:{:>2} {} {}",
                            error.code.red().bold(),
                            error.line.to_string().dimmed(),
                            error.column.to_string().dimmed(),
                            error.message,
                            error.match_str.dimmed()
                        );
                    }
                    if res.reformatted {
                        if args.check {
                            println!("{}", "File would be reformatted".yellow());
                        } else {
                            println!("{}", "File reformatted".green());
                        }
                    }
                }
            }
            Err(e) => eprintln!("{} {}", "Error:".red().bold(), e),
        }
    }

    println!(
        "\nLinted {} files, found {} errors.",
        files.len(),
        total_errors
    );

    if args.check && (total_errors > 0 || total_reformatted > 0) {
        std::process::exit(1);
    }

    Ok(())
}

fn collect_files(paths: &[PathBuf], extensions: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for path in paths {
        if path.is_file() {
            if has_extension(path, extensions) {
                files.push(path.to_path_buf());
            }
        } else if path.is_dir() {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() && has_extension(entry.path(), extensions) {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    files
}

fn has_extension(path: &Path, extensions: &[String]) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        extensions.iter().any(|e| e == ext)
    } else {
        false
    }
}

fn process_file(path: &Path, args: &Args, config: &Config) -> Result<FileResult> {
    let source = std::fs::read_to_string(path)?;
    let mut reformatted = false;
    let mut errors = Vec::new();

    if args.reformat {
        let formatted = format(config, &source);
        if formatted != source {
            reformatted = true;
            if !args.check {
                std::fs::write(path, formatted)?;
            }
        }
    }

    if args.lint {
        errors = lint(config, &source);
    }

    Ok(FileResult {
        path: path.to_path_buf(),
        errors,
        reformatted,
    })
}
