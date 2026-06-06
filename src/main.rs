//! `christen` — launch-site model and route plan for agent-namespace wiring.
//!
//! ## Usage
//!
//! ```text
//! christen plan [--format table|json] [--config <path>]
//! ```
//!
//! Prints a table (or JSON) of all discovered launch sites with their
//! classification and the action needed to route them through `agentns-claude`.
//! Exits non-zero when ≥1 site is `Unwrapped` (wrapper installed, `-wintermute` kernel).

use std::io::Write as _;
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand, ValueEnum};

use christen::{plan, ChristenConfig, FakeSource, KernelInfo, LaunchSiteSource, RawSite, SiteKind};

fn main() {
    sigpipe::reset();
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("christen: {e}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Plan(args) => run_plan(args),
    }
}

fn run_plan(args: PlanArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Load config.
    let config_path = args
        .config
        .unwrap_or_else(ChristenConfig::default_path);
    let config = ChristenConfig::load(&config_path)?;

    // Read kernel info from /proc/sys/kernel/osrelease.
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_else(|_| "unknown".to_owned())
        .trim()
        .to_owned();
    let agent_ns = std::path::Path::new("/proc/self/ns/agent").exists();
    let kernel = KernelInfo { agent_ns, release };

    // Check if wrapper is installed.
    let wrapper_installed = which_wrapper();

    // Discover sites: in production use a FakeSource populated from systemd.
    // The real SystemdSource lives in christen-route.
    // For now, scan ~/.config/systemd/user/ naively.
    let sites = discover_systemd_sites(&config);
    let source = FakeSource::new(sites);
    let raw = source.sites()?;

    // Compute the plan.
    let route_plan = plan(&raw, &kernel, wrapper_installed, &config);

    // Output.
    match args.format {
        OutputFormat::Table => print_table(&raw, &route_plan, &config)?,
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&route_plan)?;
            println!("{json}");
        }
    }

    // Exit non-zero when >=1 site is Unwrapped on a wintermute kernel with wrapper installed.
    if route_plan.to_wire > 0 {
        process::exit(2);
    }

    Ok(())
}

/// Returns `true` if `agentns-claude` is reachable on `$PATH`.
fn which_wrapper() -> bool {
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p).any(|dir| {
                dir.join("agentns-claude").exists()
                    || dir.join("agent-wrap").exists()
            })
        })
        .unwrap_or(false)
}

/// Naively discovers systemd user units from the configured directory.
/// Returns an empty list if the directory is not readable.
fn discover_systemd_sites(config: &ChristenConfig) -> Vec<RawSite> {
    let dir = &config.systemd_dir;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut sites = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".service") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        // Extract ExecStart= line.
        let exec_start = content
            .lines()
            .find(|l| l.trim_start().starts_with("ExecStart="))
            .and_then(|l| l.splitn(2, '=').nth(1))
            .unwrap_or("")
            .to_owned();
        sites.push(RawSite {
            id: name.to_owned(),
            kind: SiteKind::SystemdUnit {
                unit: name.to_owned(),
                exec_start: exec_start.clone(),
            },
            exec_line: exec_start,
        });
    }
    sites
}

/// Print a human-readable table to stdout.
fn print_table(
    raw: &[RawSite],
    route_plan: &christen::RoutePlan,
    _config: &ChristenConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    use christen::RouteAction;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    writeln!(
        out,
        "{:<35} {:<12} {:<12} {:<14} {}",
        "SITE", "KIND", "WRAP", "INTENT", "ACTION"
    )?;
    writeln!(
        out,
        "{:-<35} {:-<12} {:-<12} {:-<14} {:-<30}",
        "", "", "", "", ""
    )?;

    for action in &route_plan.actions {
        let (site_id, action_label) = match action {
            RouteAction::Wire { site, .. } => (site.as_str(), "WIRE"),
            RouteAction::Advise { site, .. } => (site.as_str(), "ADVISE"),
            RouteAction::AlreadyWrapped { site } => (site.as_str(), "ALREADY_WRAPPED"),
            RouteAction::Skip { site, .. } => (site.as_str(), "SKIP"),
        };

        // Look up corresponding raw site for kind/wrap.
        if let Some(raw_site) = raw.iter().find(|r| r.id == site_id) {
            let kind_label = match &raw_site.kind {
                SiteKind::SystemdUnit { .. } => "systemd",
                SiteKind::ShellRc { .. } => "shell-rc",
                SiteKind::Hook => "hook",
                SiteKind::Other { .. } => "other",
            };
            let wrap_label = match action {
                RouteAction::AlreadyWrapped { .. } => "wrapped",
                RouteAction::Wire { .. } => "unwrapped",
                RouteAction::Advise { .. } => "uncertain",
                RouteAction::Skip { .. } => "—",
            };
            writeln!(
                out,
                "{:<35} {:<12} {:<12} {:<14} {}",
                site_id, kind_label, wrap_label, "—", action_label
            )?;
            let _ = raw_site; // suppress unused warning
        }
    }

    writeln!(out)?;
    writeln!(
        out,
        "Summary: to_wire={} advised={} already={} skipped={}",
        route_plan.to_wire, route_plan.advised, route_plan.already, route_plan.skipped
    )?;

    Ok(())
}

#[derive(Debug, Parser)]
#[command(name = "christen", about = "Launch-site model and route plan for agent-namespace wiring")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Print the route plan for all discovered launch sites.
    Plan(PlanArgs),
}

#[derive(Debug, Parser)]
struct PlanArgs {
    /// Output format.
    #[arg(long, default_value = "table")]
    format: OutputFormat,

    /// Path to christen.toml (default: ~/.config/christen/christen.toml).
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputFormat {
    /// Human-readable table.
    Table,
    /// Machine-readable JSON.
    Json,
}
