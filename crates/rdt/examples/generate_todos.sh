#!/bin/bash

# Configuration
SERVER_URL="http://127.0.0.1:3001"
NUM_TODOS=100
UPDATES_PER_TODO=${1:-5}  # Default to 5 updates per todo, or use first argument
DEBUG=${2:-0}  # Set to 1 for debug output

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🚀 Todo Load Test Script${NC}"
echo -e "${BLUE}========================${NC}"
echo -e "Server URL: ${SERVER_URL}"
echo -e "Number of todos to create: ${NUM_TODOS}"
echo -e "Updates per todo: ${UPDATES_PER_TODO}"
if [ "$DEBUG" = "1" ]; then
    echo -e "Debug mode: ON"
fi
echo ""

# Check if server is running
echo -e "${YELLOW}Checking if server is running...${NC}"
if ! curl -s "${SERVER_URL}/todos" > /dev/null; then
    echo -e "${RED}❌ Server is not running at ${SERVER_URL}${NC}"
    echo -e "${YELLOW}Please start the server with: cargo run --example todo_server${NC}"
    exit 1
fi
echo -e "${GREEN}✅ Server is running${NC}"
echo ""

# Clear existing todos
echo -e "${YELLOW}Clearing existing todos...${NC}"
curl -s -X DELETE "${SERVER_URL}/todos" > /dev/null
echo -e "${GREEN}✅ Cleared existing todos${NC}"
echo ""

# Array to store todo IDs
declare -a todo_ids

# Create todos
echo -e "${YELLOW}Creating ${NUM_TODOS} todos...${NC}"
for i in $(seq 1 $NUM_TODOS); do
    # Generate a variety of todo titles
    titles=(
        "Learn Rust programming"
        "Build a web application"
        "Write documentation"
        "Fix bug in authentication"
        "Implement user registration"
        "Add unit tests"
        "Optimize database queries"
        "Deploy to production"
        "Review pull request"
        "Update dependencies"
        "Refactor legacy code"
        "Set up CI/CD pipeline"
        "Create API documentation"
        "Design user interface"
        "Implement caching layer"
        "Add error handling"
        "Write integration tests"
        "Configure monitoring"
        "Update README file"
        "Implement search feature"
    )
    
    # Select a random title and add number for uniqueness
    title_index=$((i % ${#titles[@]}))
    title="${titles[$title_index]} #${i}"
    
    # Create todo and extract ID
    response=$(curl -s -X POST "${SERVER_URL}/todos" \
        -H "Content-Type: application/json" \
        -d "{\"title\": \"${title}\"}")
    
    if [ "$DEBUG" = "1" ] && [ $i -le 3 ]; then
        echo "Debug: Response for todo $i: $response"
    fi
    
    # Extract todo ID from JSON response - improved extraction
    todo_id=$(echo "$response" | grep -o '"id":"[^"]*"' | sed 's/"id":"//;s/"//')
    
    if [ -n "$todo_id" ]; then
        todo_ids+=("$todo_id")
        if [ "$DEBUG" = "1" ] && [ $i -le 3 ]; then
            echo "Debug: Extracted ID: $todo_id"
        fi
        if [ $((i % 10)) -eq 0 ]; then
            echo -e "${GREEN}Created ${i}/${NUM_TODOS} todos...${NC}"
        fi
    else
        echo -e "${RED}❌ Failed to create todo ${i}${NC}"
        if [ "$DEBUG" = "1" ]; then
            echo "Debug: Failed response: $response"
        fi
    fi
done

echo -e "${GREEN}✅ Created ${#todo_ids[@]} todos${NC}"
if [ "$DEBUG" = "1" ]; then
    echo "Debug: First 5 todo IDs: ${todo_ids[@]:0:5}"
fi
echo ""

# Update todos
echo -e "${YELLOW}Making ${UPDATES_PER_TODO} updates per todo...${NC}"
total_updates=$((${#todo_ids[@]} * UPDATES_PER_TODO))
current_update=0

for todo_index in "${!todo_ids[@]}"; do
    todo_id="${todo_ids[$todo_index]}"
    
    if [ "$DEBUG" = "1" ] && [ $todo_index -lt 2 ]; then
        echo "Debug: Processing todo $todo_index with ID: $todo_id"
    fi
    
    for update_num in $(seq 1 $UPDATES_PER_TODO); do
        current_update=$((current_update + 1))
        
        # Generate different types of updates
        case $((update_num % 4)) in
            1)
                # Update title
                new_title="Updated todo ${todo_id:0:8} - revision ${update_num}"
                update_data="{\"title\": \"${new_title}\"}"
                ;;
            2)
                # Mark as completed
                update_data="{\"completed\": true}"
                ;;
            3)
                # Update title and mark as completed
                new_title="Completed todo ${todo_id:0:8} - final revision"
                update_data="{\"title\": \"${new_title}\", \"completed\": true}"
                ;;
            0)
                # Mark as not completed (reopen)
                update_data="{\"completed\": false}"
                ;;
        esac
        
        if [ "$DEBUG" = "1" ] && [ $current_update -le 5 ]; then
            echo "Debug: Update $current_update - ID: $todo_id, Data: $update_data"
        fi
        
        # Make the update (removed timeout as it's not available on all systems)
        update_response=$(curl -s -X PUT "${SERVER_URL}/todos/${todo_id}" \
            -H "Content-Type: application/json" \
            -d "$update_data" 2>/dev/null)
        
        update_exit_code=$?
        
        if [ $update_exit_code -ne 0 ]; then
            echo -e "${RED}❌ Update $current_update failed (exit code: $update_exit_code)${NC}"
            if [ "$DEBUG" = "1" ]; then
                echo "Debug: Failed update - ID: $todo_id, Data: $update_data"
            fi
            # Continue with next update instead of hanging
        elif [ "$DEBUG" = "1" ] && [ $current_update -le 5 ]; then
            echo "Debug: Update $current_update successful"
        fi
        
        # Add small delay every 10 operations to prevent overwhelming the server
        if [ $((current_update % 10)) -eq 0 ]; then
            sleep 0.1
        fi
        
        # Show progress every 25 updates
        if [ $((current_update % 25)) -eq 0 ]; then
            echo -e "${GREEN}Completed ${current_update}/${total_updates} updates...${NC}"
        fi
    done
done

echo -e "${GREEN}✅ Completed all ${total_updates} updates${NC}"
echo ""

# Get final statistics
echo -e "${YELLOW}Getting final statistics...${NC}"
stats=$(curl -s "${SERVER_URL}/todos/stats")
echo -e "${BLUE}Final Statistics:${NC}"
echo "$stats" | grep -o '"total":[0-9]*' | sed 's/"total":/Total todos: /'
echo "$stats" | grep -o '"completed":[0-9]*' | sed 's/"completed":/Completed: /'
echo "$stats" | grep -o '"remaining":[0-9]*' | sed 's/"remaining":/Remaining: /'
echo ""

echo -e "${GREEN}🎉 Load test completed successfully!${NC}"
echo -e "${YELLOW}You can view all todos at: ${SERVER_URL}/todos${NC}"
echo -e "${YELLOW}You can view stats at: ${SERVER_URL}/todos/stats${NC}" 