//! `umlcraft-cli`: UmlCraft on the command line. `umlcraft-cli mcp` is the MCP server that AI
//! clients start (on stdin and stdout); `umlcraft-cli mcp setup` registers it with them.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use serde_json::{Value, json};
use umlcraft_mcp::backend::Auto;
use umlcraft_mcp::{Access, Headless};

const USAGE: &str = "\
usage:
  umlcraft-cli mcp [--headless] [--in project.ucraft] [--connect PORT --token T]
                   [--allow-path DIR]... [--allow-any-path]
      The MCP server, on stdin and stdout. Calls go to the UmlCraft app when it runs (the
      user sees every change and can undo it), else to a project of the server's own.
      --headless        never the app
      --in FILE         start headless with this project
      --connect PORT    the app at this port of 127.0.0.1 (with --token), never headless
      --allow-path DIR  a folder in which projects and images may be opened and saved
      --allow-any-path  any folder
  umlcraft-cli mcp setup [--dry-run] [--client claude-code|claude-desktop|cursor]
                         [--allow-path DIR]... [--allow-any-path]
      Adds an `umlcraft` server with this program to each installed AI client. Without
      --allow-path the server may use the Documents folder.
  umlcraft-cli --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["--version" | "-V"] => {
            println!("umlcraft-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        ["mcp", "setup" | "install", rest @ ..] => setup(rest),
        ["mcp", rest @ ..] => serve(rest),
        [] | ["--help" | "-h" | "help"] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(format!("unknown command\n\n{USAGE}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("umlcraft-cli: {message}");
            ExitCode::FAILURE
        }
    }
}

/// Options shared by `mcp` and `mcp setup`.
#[derive(Default)]
struct Options {
    access: Access,
    headless: bool,
    input: Option<PathBuf>,
    port: Option<u16>,
    token: Option<String>,
    dry_run: bool,
    client: Option<String>,
}

fn parse(args: &[&str], setup: bool) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args.iter();
    let value = |args: &mut std::slice::Iter<&str>, flag: &str| {
        args.next().map(|v| v.to_string()).ok_or_else(|| format!("{flag} needs a value"))
    };
    while let Some(&arg) = args.next() {
        match arg {
            "--allow-path" => options.access.folders.push(PathBuf::from(value(&mut args, arg)?)),
            "--allow-any-path" => options.access.any = true,
            "--headless" if !setup => options.headless = true,
            "--in" if !setup => options.input = Some(PathBuf::from(value(&mut args, arg)?)),
            "--connect" if !setup => {
                let text = value(&mut args, arg)?;
                let port = text.rsplit(':').next().unwrap_or(&text);
                options.port = Some(port.parse().map_err(|_| format!("bad port `{text}`"))?);
            }
            "--token" if !setup => options.token = Some(value(&mut args, arg)?),
            "--dry-run" if setup => options.dry_run = true,
            "--client" if setup => options.client = Some(value(&mut args, arg)?),
            _ => return Err(format!("unknown option `{arg}`\n\n{USAGE}")),
        }
    }
    Ok(options)
}

fn serve(args: &[&str]) -> Result<(), String> {
    let mut options = parse(args, false)?;
    let mut backend = if let Some(path) = &options.input {
        let doc = umlcraft_mcp::host::open(path)?;
        // The user named the project, so its folder may be used too.
        if let Some(folder) =
            std::path::absolute(path).ok().and_then(|p| p.parent().map(Path::to_path_buf))
        {
            options.access.folders.push(folder);
        }
        Auto::headless(options.access, Headless::with(doc, Some(path.clone())))
    } else if options.headless {
        Auto::headless(options.access, Headless::new())
    } else if let Some(port) = options.port {
        let token = options.token.ok_or("--connect needs --token")?;
        Auto::connect(options.access, port, token)
    } else {
        Auto::new(options.access)
    };
    let (stdin, stdout) = (std::io::stdin(), std::io::stdout());
    umlcraft_mcp::protocol::serve(&mut backend, stdin.lock(), stdout.lock())
        .map_err(|e| e.to_string())
}

// ----- Setup ---------------------------------------------------------------------------------

fn setup(args: &[&str]) -> Result<(), String> {
    let mut options = parse(args, true)?;
    if options.access.folders.is_empty()
        && !options.access.any
        && let Some(documents) = documents_dir()
    {
        options.access.folders.push(documents);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut server_args = vec!["mcp".to_owned()];
    for folder in &options.access.folders {
        server_args.push("--allow-path".into());
        server_args.push(folder.display().to_string());
    }
    if options.access.any {
        server_args.push("--allow-any-path".into());
    }
    let wanted = |client: &str| options.client.as_deref().is_none_or(|c| c == client);
    let mut done = 0;
    if wanted("claude-code") {
        done += usize::from(claude_code(&exe, &server_args, options.dry_run)?);
    }
    if wanted("claude-desktop")
        && let Some(file) = claude_desktop_config()
    {
        let entry = json!({ "command": exe, "args": server_args });
        done +=
            usize::from(add_to_json(&file, &["mcpServers", "umlcraft"], entry, options.dry_run)?);
    }
    if wanted("cursor")
        && let Some(home) = home_dir()
        && home.join(".cursor").is_dir()
    {
        let entry = json!({ "command": exe, "args": server_args });
        let file = home.join(".cursor").join("mcp.json");
        done +=
            usize::from(add_to_json(&file, &["mcpServers", "umlcraft"], entry, options.dry_run)?);
    }
    if done == 0 {
        return Err(
            "found no AI client to set up (Claude Code, Claude Desktop, Cursor). Register it by \
             hand: command `umlcraft-cli`, arguments `mcp --allow-path <folder>`."
                .into(),
        );
    }
    if !options.dry_run {
        println!("Restart the AI client to use UmlCraft.");
    }
    Ok(())
}

/// Registers the server with Claude Code's `claude mcp add`, if `claude` is installed.
fn claude_code(exe: &Path, server_args: &[String], dry_run: bool) -> Result<bool, String> {
    let Some(claude) = ["claude", "claude.cmd"]
        .into_iter()
        .find(|c| Command::new(c).arg("--version").output().is_ok_and(|o| o.status.success()))
    else {
        return Ok(false);
    };
    let mut add = vec!["mcp", "add", "--scope", "user", "umlcraft", "--"];
    let exe = exe.display().to_string();
    add.push(&exe);
    add.extend(server_args.iter().map(String::as_str));
    if dry_run {
        println!("Claude Code: {claude} {}", add.join(" "));
        return Ok(true);
    }
    // An older entry would make `add` fail.
    let _ = Command::new(claude).args(["mcp", "remove", "--scope", "user", "umlcraft"]).output();
    let output = Command::new(claude).args(&add).output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "`claude mcp add` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    println!("Claude Code: added the server `umlcraft`.");
    Ok(true)
}

/// Claude Desktop's configuration file, if Claude Desktop is installed.
fn claude_desktop_config() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let dir = if cfg!(windows) {
        var("APPDATA")?.join("Claude")
    } else if cfg!(target_os = "macos") {
        home_dir()?.join("Library/Application Support/Claude")
    } else {
        var("XDG_CONFIG_HOME").or_else(|| home_dir().map(|h| h.join(".config")))?.join("Claude")
    };
    dir.is_dir().then(|| dir.join("claude_desktop_config.json"))
}

/// Sets `path` in the JSON file `file` to `entry`, keeping everything else.
fn add_to_json(file: &Path, path: &[&str], entry: Value, dry_run: bool) -> Result<bool, String> {
    let mut root: Value = match std::fs::read(file) {
        Ok(bytes) if !bytes.is_empty() => serde_json::from_slice(&bytes)
            .map_err(|e| format!("{}: not valid JSON ({e}); left as it is", file.display()))?,
        _ => json!({}),
    };
    let mut slot = &mut root;
    for key in &path[..path.len() - 1] {
        if !slot[*key].is_object() {
            slot[*key] = json!({});
        }
        slot = &mut slot[*key];
    }
    slot[path[path.len() - 1]] = entry;
    let text = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())? + "\n";
    if dry_run {
        println!("{}:\n{text}", file.display());
        return Ok(true);
    }
    if let Some(folder) = file.parent() {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    std::fs::write(file, text).map_err(|e| format!("{}: {e}", file.display()))?;
    println!("{}: added the server `umlcraft`.", file.display());
    Ok(true)
}

fn home_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if cfg!(windows) { var("USERPROFILE") } else { var("HOME") }
}

fn documents_dir() -> Option<PathBuf> {
    home_dir().map(|home| home.join("Documents")).filter(|dir| dir.is_dir())
}
