use super::super::repo::{
    count_feishu_bindings_for_agent, delete_displaced_default_feishu_bindings,
    delete_displaced_scoped_feishu_bindings, delete_feishu_bindings_for_agent,
    find_displaced_default_feishu_agent_ids, find_displaced_scoped_feishu_agent_ids,
    get_employee_association_row, insert_feishu_binding, list_agent_scope_rows,
    update_employee_enabled_scopes, InsertFeishuBindingInput,
};
use super::super::SaveFeishuEmployeeAssociationInput;
use super::{normalize_enabled_scopes_for_storage, resolve_employee_agent_id};
use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

async fn clear_feishu_scope_for_agent_if_unbound(
    tx: &mut Transaction<'_, Sqlite>,
    agent_id: &str,
    now: &str,
) -> Result<(), String> {
    let normalized_agent_id = agent_id.trim();
    if normalized_agent_id.is_empty() {
        return Ok(());
    }

    if count_feishu_bindings_for_agent(tx, normalized_agent_id).await? > 0 {
        return Ok(());
    }

    let employee_rows = list_agent_scope_rows(tx).await?;
    for (employee_db_id, employee_id, role_id, openclaw_agent_id, enabled_scopes_json) in
        employee_rows
    {
        let resolved_agent_id =
            resolve_employee_agent_id(&employee_id, &role_id, &openclaw_agent_id);
        if !resolved_agent_id.eq_ignore_ascii_case(normalized_agent_id) {
            continue;
        }

        let existing_scopes = serde_json::from_str::<Vec<String>>(&enabled_scopes_json)
            .unwrap_or_else(|_| vec!["app".to_string()]);
        let next_scopes = existing_scopes
            .into_iter()
            .filter(|scope| scope.trim().to_lowercase() != "feishu")
            .collect::<Vec<_>>();
        let next_scopes = normalize_enabled_scopes_for_storage(&next_scopes);
        let next_scopes_json = serde_json::to_string(&next_scopes).map_err(|e| e.to_string())?;
        update_employee_enabled_scopes(tx, &employee_db_id, &next_scopes_json, now).await?;
    }

    Ok(())
}

pub(crate) async fn save_feishu_employee_association_with_pool(
    pool: &SqlitePool,
    input: SaveFeishuEmployeeAssociationInput,
) -> Result<(), String> {
    let employee_db_id = input.employee_db_id.trim();
    if employee_db_id.is_empty() {
        return Err("employee_db_id is required".to_string());
    }

    let mode = input.mode.trim().to_lowercase();
    if mode != "default" && mode != "scoped" {
        return Err("mode must be default or scoped".to_string());
    }

    let peer_kind = input.peer_kind.trim().to_lowercase();
    if mode == "scoped" && !matches!(peer_kind.as_str(), "group" | "channel" | "direct") {
        return Err("peer_kind must be group, channel, or direct".to_string());
    }
    if mode == "scoped" && input.peer_id.trim().is_empty() {
        return Err("peer_id is required for scoped feishu association".to_string());
    }

    let employee_row = get_employee_association_row(pool, employee_db_id)
        .await?
        .ok_or_else(|| "employee not found".to_string())?;

    let existing_scopes = serde_json::from_str::<Vec<String>>(&employee_row.enabled_scopes_json)
        .unwrap_or_else(|_| vec!["app".to_string()]);
    let agent_id = resolve_employee_agent_id(
        &employee_row.employee_id,
        &employee_row.role_id,
        &employee_row.openclaw_agent_id,
    );
    if agent_id.is_empty() {
        return Err("employee is missing agent identity".to_string());
    }

    let mut next_scopes = existing_scopes;
    if input.enabled {
        next_scopes.push("feishu".to_string());
    } else {
        next_scopes.retain(|scope| scope.trim().to_lowercase() != "feishu");
    }
    let next_scopes = normalize_enabled_scopes_for_storage(&next_scopes);
    let next_scopes_json = serde_json::to_string(&next_scopes).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    let scoped_peer_id = input.peer_id.trim().to_string();

    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    update_employee_enabled_scopes(&mut tx, employee_db_id, &next_scopes_json, &now).await?;
    delete_feishu_bindings_for_agent(&mut tx, &agent_id).await?;

    let displaced_agent_ids = if input.enabled {
        if mode == "default" {
            let ids = find_displaced_default_feishu_agent_ids(&mut tx, &agent_id).await?;
            delete_displaced_default_feishu_bindings(&mut tx, &agent_id).await?;
            ids
        } else {
            let ids = find_displaced_scoped_feishu_agent_ids(
                &mut tx,
                &agent_id,
                &peer_kind,
                &scoped_peer_id,
            )
            .await?;
            delete_displaced_scoped_feishu_bindings(
                &mut tx,
                &agent_id,
                &peer_kind,
                &scoped_peer_id,
            )
            .await?;
            ids
        }
    } else {
        Vec::new()
    };

    if input.enabled {
        let binding_id = Uuid::new_v4().to_string();
        let binding_peer_kind = if mode == "default" {
            "group"
        } else {
            peer_kind.as_str()
        };
        let binding_peer_id = if mode == "default" {
            ""
        } else {
            scoped_peer_id.as_str()
        };
        let connector_meta_json =
            serde_json::to_string(&serde_json::json!({ "connector_id": "feishu" }))
                .map_err(|e| e.to_string())?;

        insert_feishu_binding(
            &mut tx,
            &InsertFeishuBindingInput {
                id: &binding_id,
                agent_id: &agent_id,
                peer_kind: binding_peer_kind,
                peer_id: binding_peer_id,
                connector_meta_json: &connector_meta_json,
                priority: input.priority,
                now: &now,
            },
        )
        .await?;
    }

    for displaced_agent_id in displaced_agent_ids {
        clear_feishu_scope_for_agent_if_unbound(&mut tx, &displaced_agent_id, &now).await?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(())
}
