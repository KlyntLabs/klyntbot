use desktop_shared::commands::{
    LabelCreateParams, LabelReorderParams, LabelUpdateParams, StatusLabelResponse,
    StatusWorkflowResponse, WorkflowCreateParams,
};
use desktop_shared::errors::ApiError;
use storage::rows::status::{StatusLabelRow, StatusWorkflowRow};

use crate::errors::map_storage_err;
use crate::state::AppCore;

// ── Row → Response converters ───────────────────────────────────────────

fn workflow_to_response(
    wf: StatusWorkflowRow,
    labels: Vec<StatusLabelRow>,
) -> StatusWorkflowResponse {
    StatusWorkflowResponse {
        id: wf.id,
        name: wf.name,
        is_template: wf.is_template,
        is_global_default: wf.is_global_default,
        labels: labels.into_iter().map(label_to_response).collect(),
    }
}

fn label_to_response(l: StatusLabelRow) -> StatusLabelResponse {
    StatusLabelResponse {
        id: l.id,
        workflow_id: l.workflow_id,
        name: l.name,
        color: l.color,
        status_group: l.status_group,
        position: l.position,
    }
}

// ── Handler methods ─────────────────────────────────────────────────────

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn workflow_list(&self) -> Result<Vec<StatusWorkflowResponse>, ApiError> {
        let workflows = self
            .repos
            .status_workflows
            .list_all()
            .await
            .map_err(map_storage_err)?;

        let mut results = Vec::with_capacity(workflows.len());
        for wf in workflows {
            let labels = self
                .repos
                .status_workflows
                .get_labels(&wf.id)
                .await
                .map_err(map_storage_err)?;
            results.push(workflow_to_response(wf, labels));
        }
        Ok(results)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn workflow_get(
        &self,
        id: String,
    ) -> Result<Option<StatusWorkflowResponse>, ApiError> {
        let wf = self
            .repos
            .status_workflows
            .get(&id)
            .await
            .map_err(map_storage_err)?;

        match wf {
            Some(wf) => {
                let labels = self
                    .repos
                    .status_workflows
                    .get_labels(&wf.id)
                    .await
                    .map_err(map_storage_err)?;
                Ok(Some(workflow_to_response(wf, labels)))
            }
            None => Ok(None),
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn workflow_get_effective(
        &self,
        project_id: Option<String>,
    ) -> Result<Vec<StatusLabelResponse>, ApiError> {
        let project_workflow_id = match project_id {
            Some(ref pid) => match self
                .repos
                .projects
                .get(pid)
                .await
                .map_err(map_storage_err)?
            {
                Some(proj) => proj.workflow_id,
                None => None,
            },
            None => None,
        };
        let labels = self
            .repos
            .status_workflows
            .get_effective_labels(project_workflow_id.as_deref())
            .await
            .map_err(map_storage_err)?;

        Ok(labels.into_iter().map(label_to_response).collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn workflow_create(
        &self,
        params: WorkflowCreateParams,
    ) -> Result<StatusWorkflowResponse, ApiError> {
        let wf = match params.source_workflow_id {
            Some(source_id) => self
                .repos
                .status_workflows
                .duplicate(&source_id, &params.name)
                .await
                .map_err(map_storage_err)?,
            None => self
                .repos
                .status_workflows
                .create(&params.name, params.is_template.unwrap_or(false))
                .await
                .map_err(map_storage_err)?,
        };

        let labels = self
            .repos
            .status_workflows
            .get_labels(&wf.id)
            .await
            .map_err(map_storage_err)?;

        Ok(workflow_to_response(wf, labels))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn workflow_delete(&self, id: String) -> Result<bool, ApiError> {
        self.repos
            .status_workflows
            .delete(&id)
            .await
            .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn label_create(
        &self,
        params: LabelCreateParams,
    ) -> Result<StatusLabelResponse, ApiError> {
        let position = params.position.unwrap_or(0);

        let label = self
            .repos
            .status_workflows
            .add_label(
                &params.workflow_id,
                &params.name,
                &params.color,
                &params.status_group,
                position,
            )
            .await
            .map_err(map_storage_err)?;

        Ok(label_to_response(label))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn label_update(
        &self,
        params: LabelUpdateParams,
    ) -> Result<StatusLabelResponse, ApiError> {
        let label = self
            .repos
            .status_workflows
            .update_label(
                &params.id,
                params.name.as_deref(),
                params.color.as_deref(),
                params.status_group.as_deref(),
                params.position,
            )
            .await
            .map_err(map_storage_err)?;

        Ok(label_to_response(label))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn label_delete(&self, id: String) -> Result<bool, ApiError> {
        self.repos
            .status_workflows
            .delete_label(&id)
            .await
            .map_err(map_storage_err)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn label_reorder(&self, params: LabelReorderParams) -> Result<(), ApiError> {
        self.repos
            .status_workflows
            .reorder_labels(&params.workflow_id, &params.label_ids)
            .await
            .map_err(map_storage_err)
    }
}
