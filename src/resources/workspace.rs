use crate::api::workspace as workspace_api;
use crate::client::Honcho;
use crate::error::Error;
use crate::models::page::Page;
use crate::models::workspace::{Workspace, WorkspaceCreate, WorkspaceListOptions, WorkspaceUpdate};

/// Handle for the workspace-level Workspaces API.
///
/// Unlike peers and sessions, workspaces are not scoped to a single workspace
/// id: the workspace is resolved from the caller's JWT. Use it through
/// `Honcho::workspaces`.
pub struct Workspaces {
    client: Honcho,
}

impl Workspaces {
    pub fn new(client: Honcho) -> Self {
        Self { client }
    }

    /// Get or create a workspace by id. If the workspace doesn't exist, it's
    /// created with the supplied metadata and configuration.
    pub async fn get_or_create(&self, create: WorkspaceCreate) -> Result<Workspace, Error> {
        workspace_api::get_or_create_workspace(&self.client, &create).await
    }

    /// Get a workspace by id.
    pub async fn get(&self, workspace_id: impl Into<String>) -> Result<Workspace, Error> {
        workspace_api::get_workspace(&self.client, &workspace_id.into()).await
    }

    /// List all workspaces in this account, paginated.
    pub async fn list(&self, opts: &WorkspaceListOptions) -> Result<Page<Workspace>, Error> {
        workspace_api::list_workspaces(&self.client, opts).await
    }

    /// Update a workspace's metadata and/or configuration.
    pub async fn update(
        &self,
        workspace_id: impl Into<String>,
        update: WorkspaceUpdate,
    ) -> Result<Workspace, Error> {
        workspace_api::update_workspace(&self.client, &workspace_id.into(), &update).await
    }

    /// Delete a workspace. Fails with a 409 if any sessions are still active.
    pub async fn delete(&self, workspace_id: impl Into<String>) -> Result<(), Error> {
        workspace_api::delete_workspace(&self.client, &workspace_id.into()).await
    }
}
