use desktop_shared::commands::{ObjectiveResponse, ProjectResponse, TaskResponse};

#[tauri::command]
pub async fn task_list() -> Result<Vec<TaskResponse>, String> {
    // Stubbed — frontend uses mock data directly for now
    Ok(vec![])
}

#[tauri::command]
pub async fn project_list() -> Result<Vec<ProjectResponse>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn objective_list() -> Result<Vec<ObjectiveResponse>, String> {
    Ok(vec![])
}
