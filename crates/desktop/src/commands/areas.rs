use desktop_shared::commands::AreaResponse;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn area_list(state: State<'_, AppCore>) -> Result<Vec<AreaResponse>, String> {
    let areas = state
        .repos
        .areas
        .list(Some("active"))
        .await
        .map_err(|e| e.to_string())?;

    let mut results = Vec::with_capacity(areas.len());
    for a in &areas {
        let project_count = state
            .repos
            .areas
            .count_projects(&a.id)
            .await
            .map_err(|e| e.to_string())?;
        let task_count = state
            .repos
            .areas
            .count_actions(&a.id)
            .await
            .map_err(|e| e.to_string())?;

        results.push(AreaResponse {
            id: a.id.clone(),
            name: a.name.clone(),
            color: a.color.clone(),
            icon: a.icon.clone(),
            project_count,
            task_count,
        });
    }
    Ok(results)
}
