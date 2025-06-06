# RDT Examples

This directory contains examples demonstrating how to use the RDT (Replicated Data Types) crate.

## Todo Server Example

The `todo_server.rs` example demonstrates a complete Axum-based REST API server with real-time WebSocket synchronization using RDT.

### Features

- **REST API**: Full CRUD operations for todos
- **Real-time sync**: WebSocket support for live updates across clients
- **RDT Integration**: Uses RDT's document maps for thread-safe state management
- **Statistics**: Get todo completion statistics

### Running the Example

```bash
# From the workspace root
cargo run --example todo_server --package rdt

# Or from the rdt crate directory
cd crates/rdt
cargo run --example todo_server
```

### API Endpoints

Once running on `http://127.0.0.1:3001`:

- `GET /todos` - List all todos
- `POST /todos` - Create a new todo
  ```json
  { "title": "Learn RDT" }
  ```
- `GET /todos/{id}` - Get a specific todo
- `PUT /todos/{id}` - Update a todo
  ```json
  { "title": "Updated title", "completed": true }
  ```
- `DELETE /todos/{id}` - Delete a todo
- `DELETE /todos` - Clear all todos
- `GET /todos/stats` - Get todo statistics

### WebSocket Real-time Sync

Connect to `ws://127.0.0.1:3001/ws` for real-time updates.

Example WebSocket messages:

**Subscribe to todo updates:**

```json
{
  "type": "Subscribe",
  "document_id": "todos",
  "map_key": "todos"
}
```

**Get full state:**

```json
{
  "type": "GetFullState",
  "document_id": "todos",
  "map_key": "todos"
}
```

The server will automatically broadcast changes to all subscribed clients when todos are created, updated, or deleted via the REST API.

### Load Testing with the Generation Script

The `generate_todos.sh` script creates 100 todos and performs multiple updates on each, perfect for testing RDT's synchronization capabilities under load.

**Usage:**

```bash
# First, start the todo server
cargo run --example todo_server

# In another terminal, run the load test (default: 5 updates per todo)
./examples/generate_todos.sh

# Or specify a custom number of updates per todo
./examples/generate_todos.sh 10
```

**What the script does:**

- ✅ Verifies the server is running
- 🧹 Clears any existing todos
- 📝 Creates 100 todos with varied, realistic titles
- 🔄 Makes n updates per todo (title changes, completion status toggles)
- 📊 Shows progress and final statistics
- 🎯 Demonstrates RDT's performance under concurrent operations

**Script Features:**

- **Colorful output** with progress indicators
- **Error handling** with server connectivity checks
- **Varied operations** including title updates and completion toggles
- **Real-time progress** updates during execution
- **Final statistics** showing total, completed, and remaining todos

This is excellent for testing how RDT handles:

- High-frequency updates to the same document
- Multiple concurrent operations
- Real-time synchronization performance
- WebSocket broadcast efficiency

### Testing with curl

```bash
# Create a todo
curl -X POST http://127.0.0.1:3001/todos \
  -H "Content-Type: application/json" \
  -d '{"title": "Learn RDT"}'

# List todos
curl http://127.0.0.1:3001/todos

# Get stats
curl http://127.0.0.1:3001/todos/stats
```
