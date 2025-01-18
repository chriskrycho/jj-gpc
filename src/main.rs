use std::process;

use clap::Parser as _;
use ollama_rs::{
    generation::{
        completion::{request::GenerationRequest, GenerationResponse},
        options::GenerationOptions,
        parameters::{FormatType, JsonSchema, JsonStructure},
    },
    Ollama,
};
use serde::Deserialize;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();

    let args = Cli::parse();
    let revset = format!("{}..{}", args.from, args.change);

    let log_template = match args.log_format {
        Some(LogFormat::Full) => LOG_FULL,
        Some(LogFormat::OneLine) | None => LOG_ONE_LINE,
    };

    let commits = execute(process::Command::new("jj").args(&[
        "log",
        "-T",
        log_template,
        "-r",
        &revset,
        "--no-graph",
    ]));

    if commits.stdout.trim().is_empty() {
        eprintln!("No commits to summarize");
        std::process::exit(1);
    }

    let prompt = format!(
        "{PROMPT_START}\n\n{commits}\n\n{PROMPT_END}",
        commits = commits.stdout
    );
    log::debug!("prompt: {prompt}");

    let request = GenerationRequest::new(args.model.clone(), prompt.clone())
        .format(FormatType::StructuredJson(JsonStructure::new::<Branch>()))
        .options(
            GenerationOptions::default()
                .top_k(args.top_k)
                .top_p(args.top_p)
                .temperature(args.temperature),
        );

    let response_result = Ollama::default()
        .generate(request)
        .await
        .unwrap_or_else(|e| panic!("{e}"));

    let GenerationResponse { response, .. } = response_result;

    let Branch(branch) = serde_json::from_str::<Branch>(&response).unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(1);
    });

    let branch_name = args
        .prefix
        .map(|prefix| format!("{prefix}/{}", branch))
        .unwrap_or(branch);

    if args.dry_run {
        println!(
            "[dry run] jj bookmark create {branch_name} --revision {}",
            args.change
        );
        println!("[dry run] jj git push --bookmark {branch_name}");
        return;
    }

    println!(
        "jj bookmark create {branch_name} --revision {}",
        args.change
    );
    let branch_output = execute(process::Command::new("jj").args(&[
        "bookmark",
        "create",
        &branch_name,
        "--revision",
        &args.change,
    ]));

    if !branch_output.stdout.trim().is_empty() {
        println!("{}", branch_output.stdout);
    }

    if !branch_output.stderr.trim().is_empty() {
        println!("{}", branch_output.stderr);
    }

    println!("jj git push --bookmark {branch_name} --allow-new");
    let push_output = execute(process::Command::new("jj").args(&[
        "git",
        "push",
        "--bookmark",
        &branch_name,
        "--allow-new",
    ]));
    push_output.to_console();
}

#[repr(transparent)]
#[derive(JsonSchema, Deserialize, Debug)]
struct Branch(#[schemars(regex(pattern = "^[a-z]{1,10}+(-[a-z]{1,10}){2,4}$"))] String);

fn execute(command: &mut process::Command) -> CommandOutput {
    log::trace!("{command:?}");
    let process::Output {
        status,
        stdout,
        stderr,
    } = command
        .output()
        .unwrap_or_else(|e| panic!("Could not execute command {command:?}.\nCause: {e}"));

    if !status.success() {
        eprintln!(
            "Error running '{command:?}' ({status}):\nCause: {}",
            String::from_utf8_lossy(&stderr)
        );
        process::exit(status.code().unwrap_or(1));
    }

    CommandOutput {
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
    }
}

struct CommandOutput {
    stdout: String,
    stderr: String,
}

impl CommandOutput {
    fn to_console(&self) {
        let Self { stdout, stderr } = self;
        if !stdout.trim().is_empty() {
            print!("{}", stdout);
        }

        if !stderr.trim().is_empty() {
            eprint!("{}", stderr);
        }
    }
}

/// Generate a branch name for use with jj.
#[derive(clap::Parser, Debug)]
#[command(version, author)]
struct Cli {
    #[arg(default_value = "@")]
    change: String,

    #[arg(long, value_enum)]
    log_format: Option<LogFormat>,

    #[arg(short, long, default_value = "trunk()")]
    from: String,

    /// Prefix for the generated branch name, `<prefix>/<generated>`
    #[arg(short, long)]
    prefix: Option<String>,

    /// Generate the branch name, but do not actually create or push it.
    #[arg(long = "dry-run", default_value = "false")]
    dry_run: bool,

    /// The temperature of the model. Increasing the temperature will make the
    /// model answer more creatively.
    #[arg(long, default_value = "2")]
    temperature: f32,

    /// Reduces the probability of generating nonsense. A higher value (e.g. 100)
    /// will give more diverse answers, while a lower value (e.g. 10) will be more
    /// conservative. (Default: 40)
    #[arg(long, default_value = "20")]
    top_k: u32,

    /// Works together with top-k. A higher value (e.g., 0.95) will lead to more
    /// diverse text, while a lower value (e.g., 0.5) will generate more focused
    /// and conservative text.
    #[arg(long, default_value = "0.7")]
    top_p: f32,

    /// Which model to use. Can be any model available in Ollama on your system.
    ///
    /// The model you choose to use will significantly alters the quality of the
    /// output, so you may need to tune the parameters as well. If this is not a
    /// model available in Ollama on your system, the request will fail.
    #[arg(long, default_value = "phi3")]
    model: String,
}

#[derive(clap::ValueEnum, Debug, Clone)]
enum LogFormat {
    OneLine,
    Full,
}

const LOG_ONE_LINE: &'static str =
    r#""```\n" ++ if(description, description.first_line(), '') ++ "\n```\n\n---\n\n""#;
const LOG_FULL: &'static str = r#""```\n" ++ if(description, description, '') ++ "```\n\n---\n\n""#;

const PROMPT_START: &'static str = r#"Rules for branch names:

- A good branch name uses at least 3 and no more than 5 lowercase words separated by hyphens.
- A good branch name is always derived from the summary of changes in the log.
- A good branch name incorporates the sentiment of the majority of commits in the log.
- A bad branch name has too little information, like `ab-1234`.
- A bad branch name would only use the date.
- Empty commits are not relevant.

Commits are in blocks of markdown, separated by `---`. The commits that make up this branch are:
"#;
const PROMPT_END: &'static str = r#"

The best descriptive branch name for these commits (*not* a pull request description, just a branch name) for a Git branch containing these commits is:
"#;
