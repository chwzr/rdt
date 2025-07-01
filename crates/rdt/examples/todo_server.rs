use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use rdt::{DocumentStore, RdtState};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, Level};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTodoRequest {
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTodoRequest {
    pub title: Option<String>,
    pub completed: Option<bool>,
}

// REST API handlers
async fn get_todos(State(rdt_state): State<RdtState>) -> Result<Json<Vec<Todo>>, StatusCode> {
    let document = rdt_state.store().create_document("todos".to_string());
    let todos_map = document.create_map("todos".to_string());

    let todos: Vec<Todo> = todos_map
        .iter()
        .filter_map(|entry| serde_json::from_value(entry.value().clone()).ok())
        .collect();

    Ok(Json(todos))
}

async fn create_todo(
    State(rdt_state): State<RdtState>,
    Json(request): Json<CreateTodoRequest>,
) -> Result<Json<Todo>, StatusCode> {
    let document = rdt_state.store().create_document("todos".to_string());
    let todos_map = document.create_map("todos".to_string());

    let todo = Todo {
        id: Uuid::new_v4().to_string(),
        title: request.title,
        completed: false,
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let todo_json = serde_json::to_value(&todo).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    todos_map.insert(todo.id.clone(), todo_json);

    info!("Created todo: {}", todo.id);
    Ok(Json(todo))
}

async fn get_todo(
    State(rdt_state): State<RdtState>,
    Path(id): Path<String>,
) -> Result<Json<Todo>, StatusCode> {
    let document = rdt_state.store().create_document("todos".to_string());
    let todos_map = document.create_map("todos".to_string());

    let todo_value = todos_map.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let todo: Todo = serde_json::from_value(todo_value.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(todo))
}

async fn update_todo(
    State(rdt_state): State<RdtState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateTodoRequest>,
) -> Result<Json<Todo>, StatusCode> {
    let document = rdt_state.store().create_document("todos".to_string());
    let todos_map = document.create_map("todos".to_string());

    let todo_value = todos_map.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let mut todo: Todo = serde_json::from_value(todo_value.clone())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(title) = request.title {
        todo.title = title;
    }
    if let Some(completed) = request.completed {
        todo.completed = completed;
    }

    let todo_json = serde_json::to_value(&todo).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    todos_map.insert(id.clone(), todo_json);

    info!("Updated todo: {}", id);
    Ok(Json(todo))
}

async fn delete_todo(
    State(rdt_state): State<RdtState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let document = rdt_state.store().create_document("todos".to_string());
    let todos_map = document.create_map("todos".to_string());

    todos_map.remove(&id).ok_or(StatusCode::NOT_FOUND)?;

    info!("Deleted todo: {}", id);
    Ok(StatusCode::NO_CONTENT)
}

async fn clear_todos(State(rdt_state): State<RdtState>) -> Result<StatusCode, StatusCode> {
    let document = rdt_state.store().create_document("todos".to_string());
    let todos_map = document.create_map("todos".to_string());

    // Get all todo IDs first, then remove them individually to avoid broadcast bottleneck
    let todo_ids: Vec<String> = todos_map.iter().map(|entry| entry.key().clone()).collect();

    let count = todo_ids.len();

    // Remove todos individually instead of using clear() to avoid large batch broadcasts
    for id in todo_ids {
        todos_map.remove(&id);
    }

    info!("Cleared {} todos individually", count);
    Ok(StatusCode::NO_CONTENT)
}

async fn get_stats(
    State(rdt_state): State<RdtState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let document = rdt_state.store().create_document("todos".to_string());
    let todos_map = document.create_map("todos".to_string());

    let mut completed_count = 0;
    let mut total_count = 0;

    for entry in todos_map.iter() {
        if let Ok(todo) = serde_json::from_value::<Todo>(entry.value().clone()) {
            total_count += 1;
            if todo.completed {
                completed_count += 1;
            }
        }
    }

    let stats = json!({
        "total": total_count,
        "completed": completed_count,
        "remaining": total_count - completed_count
    });

    Ok(Json(stats))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting Todo Server with RDT...");

    // Create document store
    let store = Arc::new(DocumentStore::new());

    // Create RDT state
    let rdt_state = RdtState::new(store);

    // Create router with RDT WebSocket support
    let app = rdt::router_with_rdt_state(rdt_state.clone())
        .route(
            "/todos",
            get(get_todos).post(create_todo).delete(clear_todos),
        )
        .route(
            "/todos/{id}",
            get(get_todo).put(update_todo).delete(delete_todo),
        )
        .route("/todos/stats", get(get_stats))
        .with_state(rdt_state);

    // Start server
    let listener = TcpListener::bind("127.0.0.1:3001").await?;
    info!("Server running on http://127.0.0.1:3001");
    info!("WebSocket endpoint available at ws://127.0.0.1:3001/ws");
    info!("API endpoints:");
    info!("  GET    /todos        - List all todos");
    info!("  POST   /todos        - Create a new todo");
    info!("  GET    /todos/:id    - Get a specific todo");
    info!("  PUT    /todos/:id    - Update a todo");
    info!("  DELETE /todos/:id    - Delete a todo");
    info!("  DELETE /todos        - Clear all todos");
    info!("  GET    /todos/stats  - Get todo statistics");

    axum::serve(listener, app).await?;

    Ok(())
}
