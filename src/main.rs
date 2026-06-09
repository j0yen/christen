//! `christen` — launch-site model and route plan for agent-namespace wiring.
//!
//! ## Usage
//!
//! ```text
//! christen plan  [--format table|json] [--config <path>]
//! christen cap   [--verify] [--format table|json]
//! christen probe [--pid N] [--emit] [--format json]
//! christen route [--apply] [--unit <name>] [--config <path>]
//! ```
//!
//! `plan` prints a table (or JSON) of all discovered launch sites with their
//! classification and the action needed to route them through `agentns-claude`.
//! Exits non-zero when ≥1 site is `Unwrapped` (wrapper installed, `-wintermute` kernel).
//!
//! `probe` reads the `/proc` agent-namespace surface for the current process
//! (or a target PID) and classifies it as `init` / `live` / `absent` / `malformed`.
//! With `--emit`, also applies the docket edge-trigger via the `docket` CLI.
//!
//! `route` prints the drop-in files that would route each systemd unit through
//! `agentns-claude`. With `--apply`, writes the drop-ins (never runs
//! `daemon-reload` or `restart`).

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand, ValueEnum};

use christen::{
    cap_plan, default_launcher_paths, verify_cap, CapPlanEntry, CapReader, GetcapReader,
    VerifyResult, SCOPE_EXPLAINER,
};
use christen::{
    apply_docket_op, apply_route, classify, plan, verdict, ChristenConfig, FakeSource, KernelInfo,
    LaunchSiteSource, NsState, ProbeOutput, ProcReader, RawSite, RealProcReader, SiteKind,
    SystemdSource,
};

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
        Commands::Cap(args) => run_cap(&args),
        Commands::Probe(args) => run_probe(args),
        Commands::Route(args) => run_route(args),
    }
}

// ── christen cap ──────────────────────────────────────────────────────────────

fn run_cap(args: &CapArgs) -> Result<(), Box<dyn std::error::Error>> {
    let reader = GetcapReader;
    let binaries = collect_launcher_caps(&reader);
    let cap_plan_result = cap_plan(&binaries);

    match args.format {
        OutputFormat::Json => print_cap_json(&cap_plan_result)?,
        OutputFormat::Table => print_cap_table(&cap_plan_result)?,
    }

    if args.verify {
        print_cap_verify(&binaries)?;
    }

    Ok(())
}

type CapBinaries = Vec<(PathBuf, christen::CapState)>;

fn collect_launcher_caps(reader: &dyn CapReader) -> CapBinaries {
    let (agentns, agent_wrap) = default_launcher_paths();
    let mut binaries: CapBinaries = Vec::new();
    if let Some(p) = agentns {
        let state = reader.caps(&p);
        binaries.push((p, state));
    }
    if let Some(p) = agent_wrap {
        let state = reader.caps(&p);
        binaries.push((p, state));
    }
    if binaries.is_empty() {
        eprintln!(
            "christen cap: neither agentns-claude nor agent-wrap found on PATH.\n\
             Install the agent-namespace tools first."
        );
        process::exit(1);
    }
    binaries
}

fn print_cap_json(plan: &christen::CapPlan) -> Result<(), Box<dyn std::error::Error>> {
    let out = serde_json::json!({
        "scope_explainer": SCOPE_EXPLAINER,
        "binaries": plan.entries.iter().map(cap_entry_to_json).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn cap_entry_to_json(e: &CapPlanEntry) -> serde_json::Value {
    match e {
        CapPlanEntry::Grant { path, command } => serde_json::json!({
            "path": path, "state": "absent", "action": "grant", "command": command
        }),
        CapPlanEntry::AlreadyGranted { path } => serde_json::json!({
            "path": path, "state": "present", "action": "already_granted"
        }),
        CapPlanEntry::Blocked { path, reason } => serde_json::json!({
            "path": path, "state": "blocked", "action": "blocked", "reason": reason
        }),
    }
}

fn print_cap_table(plan: &christen::CapPlan) -> Result<(), Box<dyn std::error::Error>> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "{SCOPE_EXPLAINER}")?;
    for entry in &plan.entries {
        match entry {
            CapPlanEntry::Grant { path, command } => {
                writeln!(out, "  ABSENT  {}", path.display())?;
                writeln!(out, "    Run:  {command}")?;
            }
            CapPlanEntry::AlreadyGranted { path } => {
                writeln!(out, "  PRESENT {}", path.display())?;
                writeln!(out, "    (`cap_sys_admin+ep` already granted — no action needed)")?;
            }
            CapPlanEntry::Blocked { path, reason } => {
                writeln!(out, "  BLOCKED {}", path.display())?;
                writeln!(out, "    Reason: {reason}")?;
            }
        }
        writeln!(out)?;
    }
    Ok(())
}

fn print_cap_verify(
    binaries: &[(PathBuf, christen::CapState)],
) -> Result<(), Box<dyn std::error::Error>> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    writeln!(out, "=== --verify ===")?;
    for (path, _) in binaries {
        let result = verify_cap(path);
        match &result {
            VerifyResult::Live { session_id } => {
                writeln!(out, "  LIVE  {}: agent_session = {session_id}", path.display())?;
            }
            VerifyResult::EpermFallback => {
                writeln!(
                    out,
                    "  EPERM-FALLBACK  {}: agent_session is all-zeros (cap not yet granted)",
                    path.display()
                )?;
            }
            VerifyResult::Absent => {
                writeln!(out, "  ABSENT  {}: kernel does not have CONFIG_AGENT_NS=y", path.display())?;
            }
            VerifyResult::Error { detail } => {
                writeln!(out, "  ERROR  {}: {detail}", path.display())?;
            }
        }
    }
    Ok(())
}

// ── christen probe ────────────────────────────────────────────────────────────

fn run_probe(args: ProbeArgs) -> Result<(), Box<dyn std::error::Error>> {
    let reader = RealProcReader;
    let reading = reader.read(args.pid)?;
    let kernel_is_wintermute = reading.kernel_is_wintermute;
    let state = classify(&reading);
    let v = verdict(&state, kernel_is_wintermute);

    let output = ProbeOutput::from_state_verdict(&state, &v);

    // Print output.
    match args.format {
        ProbeFormat::Text => {
            let state_label = match &state {
                NsState::Absent => "absent",
                NsState::Init { .. } => "init",
                NsState::Live { .. } => "live",
                NsState::Malformed { .. } => "malformed",
            };
            println!("state: {state_label}");
            println!("ok:    {}", v.ok);
            println!("{}", v.prose);
        }
        ProbeFormat::Json => {
            let json = serde_json::to_string_pretty(&output)?;
            println!("{json}");
        }
    }

    // Apply docket op if requested.
    if args.emit {
        apply_docket_op(&v.docket)?;
    }

    // Exit non-zero on fault states (Absent or Malformed on wintermute kernel).
    if !v.ok {
        process::exit(1);
    }

    Ok(())
}

// ── christen plan ─────────────────────────────────────────────────────────────

fn run_plan(args: PlanArgs) -> Result<(), Box<dyn std::error::Error>> {
    // Load config.
    let config_path = args.config.unwrap_or_else(ChristenConfig::default_path);
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
        OutputFormat::Table => print_table(&raw, &route_plan)?,
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
    std::env::var_os("PATH").is_some_and(|p| {
        std::env::split_paths(&p).any(|dir| {
            dir.join("agentns-claude").exists() || dir.join("agent-wrap").exists()
        })
    })
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
        // Extract ExecStart= line (use split_once to avoid manual_split_once lint).
        let exec_start = content
            .lines()
            .find(|l| l.trim_start().starts_with("ExecStart="))
            .and_then(|l| l.split_once('=').map(|x| x.1))
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
) -> Result<(), Box<dyn std::error::Error>> {
    use christen::RouteAction;

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    writeln!(
        out,
        "{:<35} {:<12} {:<12} {:<14} ACTION",
        "SITE", "KIND", "WRAP", "INTENT"
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

// ── christen route ────────────────────────────────────────────────────────────

fn run_route(args: RouteArgs) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = args.config.unwrap_or_else(ChristenConfig::default_path);
    let config = ChristenConfig::load(&config_path)?;

    // Read kernel info.
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .unwrap_or_else(|_| "unknown".to_owned())
        .trim()
        .to_owned();
    let agent_ns = std::path::Path::new("/proc/self/ns/agent").exists();
    let kernel = KernelInfo { agent_ns, release };
    let wrapper_installed = which_wrapper();

    // Discover sites via SystemdSource (includes synthetic ShellRc site).
    let source = SystemdSource::new(config.systemd_dir.clone());
    let raw = source.sites()?;

    // Compute the plan.
    let route_plan = plan(&raw, &kernel, wrapper_installed, &config);

    // Apply or print.
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    apply_route(
        &route_plan,
        &config,
        !args.apply,
        args.unit.as_deref(),
        &mut out,
    )?;

    Ok(())
}

// ── CLI types ─────────────────────────────────────────────────────────────────

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
    /// Detect whether launcher binaries carry `cap_sys_admin+ep`; print the exact
    /// `sudo setcap` line needed (never executes it).
    Cap(CapArgs),
    /// Probe the /proc agent-namespace surface and classify the state.
    Probe(ProbeArgs),
    /// Print (or write with --apply) systemd drop-in overrides that route each
    /// launch site through agentns-claude. Never runs daemon-reload or restart.
    Route(RouteArgs),
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

#[derive(Debug, Parser)]
struct CapArgs {
    /// After detecting cap state, verify by spawning the launcher under sbx
    /// and reading back the child's `agent_session` (read-only; no setcap).
    #[arg(long)]
    verify: bool,

    /// Output format.
    #[arg(long, default_value = "table")]
    format: OutputFormat,
}

#[derive(Debug, Parser)]
struct ProbeArgs {
    /// Target PID to probe (default: current process).
    #[arg(long)]
    pid: Option<u32>,

    /// Apply the docket edge-trigger (shell `docket` with the mapped op).
    #[arg(long)]
    emit: bool,

    /// Output format.
    #[arg(long, default_value = "text")]
    format: ProbeFormat,
}

#[derive(Debug, Parser)]
struct RouteArgs {
    /// Write drop-in files to disk; print (but do not run) daemon-reload lines.
    #[arg(long)]
    apply: bool,

    /// Restrict to a single unit name (e.g. `claude-build.service`).
    #[arg(long)]
    unit: Option<String>,

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

#[derive(Debug, Clone, ValueEnum)]
enum ProbeFormat {
    /// Human-readable text.
    Text,
    /// Machine-readable JSON.
    Json,
}
