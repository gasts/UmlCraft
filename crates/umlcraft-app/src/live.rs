//! The running app's side of the MCP server (`umlcraft-cli mcp`): calls arrive from
//! [`umlcraft_mcp::live::LiveServer`] on its threads, cross to the main thread through a
//! channel and run there against the open project, like the user's own edits: one undo step
//! each, shown at once.

use std::path::{Path, PathBuf};

use rust_i18n::t;
use umlcraft_mcp::Host;
use umlcraft_mcp::live::{LiveServer, Request};
use umlcraft_model::{Document, Id};
use umlcraft_notation::scene::TextMeasure;

use crate::workspace::Workspace;

/// Starts listening for the MCP server and returns the server (which stops announcing itself
/// when dropped) and where its calls arrive. `None` where there is no folder for local data.
pub fn start() -> Option<(LiveServer, async_channel::Receiver<Request>)> {
    let (tx, rx) = async_channel::unbounded();
    let server = LiveServer::start(move |request| {
        // The app is quitting when nobody receives.
        let _ = tx.send_blocking(request);
    })
    .ok()?;
    Some((server, rx))
}

/// What a call asks of the main view beyond the workspace.
#[derive(Default)]
pub struct Effects {
    /// A project to open instead of the current one.
    pub replace: Option<(Document, Option<PathBuf>)>,
    /// The file the project was saved to, for the recent files.
    pub saved: Option<PathBuf>,
}

/// The open project as the tools see it.
pub struct AppHost<'a> {
    pub ws: &'a mut Workspace,
    pub effects: &'a mut Effects,
}

impl Host for AppHost<'_> {
    fn document(&self) -> &Document {
        &self.ws.doc
    }

    fn parts(&mut self) -> (&mut Document, &dyn TextMeasure) {
        (&mut self.ws.doc, &*self.ws.measure)
    }

    fn label(&self, tool: &str) -> String {
        t!("edit.mcp", tool = tool).to_string()
    }

    fn path(&self) -> Option<&Path> {
        self.ws.path.as_deref()
    }

    fn live(&self) -> bool {
        true
    }

    fn active_diagram(&self) -> Option<Id> {
        self.ws.active_diagram
    }

    fn selection(&self) -> Vec<Id> {
        self.ws.selection.clone()
    }

    fn show_diagram(&mut self, diagram: Id) {
        self.ws.open_diagram(diagram);
    }

    fn replace(&mut self, doc: Document, path: Option<PathBuf>) -> Result<(), String> {
        if self.ws.doc.is_dirty() {
            return Err("The project open in UmlCraft has unsaved changes. Ask the user to save \
                        or discard them first."
                .into());
        }
        self.effects.replace = Some((doc, path));
        Ok(())
    }

    fn saved(&mut self, path: PathBuf) {
        self.ws.path = Some(path.clone());
        self.effects.saved = Some(path);
    }
}
