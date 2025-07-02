#!/bin/bash

# ===================================================================
#             START-AND-MONITOR SCRIPT FOR APPLICATIONS
# ===================================================================
# This script starts a backend server, waits for it to be ready,
# starts a frontend GUI, and then monitors the frontend.
# When the frontend process is closed, it automatically stops the backend.
# ===================================================================


# --- Configuration ---
# Adjust these paths to your executable locations
BACKEND_PATH="./opossum_backend"
FRONTEND_PATH="./opossum_gui"

# --- Process & Network Configuration ---
BACKEND_PORT=8001 # The port your backend listens on for the readiness check

# --- Timeout Configuration ---
STARTUP_TIMEOUT_SECONDS=30 # How long to wait for the backend to start


# ===================================================================
#                           SCRIPT LOGIC
#             (No changes needed below this line)
# ===================================================================

# Globally defined variable for the backend process ID
BACKEND_PID=

# --- Cleanup function to be called on script exit ---
cleanup() {
    echo "Performing cleanup..."
    # Check if the backend process ID is set and the process exists
    if [ ! -z "$BACKEND_PID" ] && ps -p $BACKEND_PID > /dev/null; then
        echo "Stopping backend server (PID: $BACKEND_PID)..."
        # Terminate the process and all its children
        kill -- -$(ps -o pgid= $BACKEND_PID | grep -o '[0-9]*')
    fi
}

# Trap the EXIT signal (normal exit, errors, Ctrl+C) to run the cleanup function
trap cleanup EXIT


# --- 1. Start the Backend Server ---
echo "Starting Backend Server..."
# Start the backend in the background
$BACKEND_PATH &
# Capture its Process ID (PID)
BACKEND_PID=$!

# A brief pause to allow the process to potentially fail immediately
sleep 0.5

# Check if the process is still running
if ! ps -p $BACKEND_PID > /dev/null; then
    echo "Error: Failed to start backend at '$BACKEND_PATH'." >&2
    exit 1
fi
echo "Backend process started with PID: $BACKEND_PID"


# --- 2. Wait for the Backend to be Ready ---
echo "Waiting for backend to be available on port $BACKEND_PORT..."
elapsed=0
# This loop uses Bash's built-in /dev/tcp to check the port.
# It attempts to open a connection; success (exit code 0) means the port is ready.
while ! (echo > /dev/tcp/localhost/$BACKEND_PORT) &>/dev/null; do
    if [ $elapsed -ge $STARTUP_TIMEOUT_SECONDS ]; then
        echo "Error: Timeout after $STARTUP_TIMEOUT_SECONDS seconds. Backend is not ready." >&2
        # The 'trap' on EXIT will handle killing the process.
        exit 1
    fi
    sleep 1
    ((elapsed++))
done
echo "Backend is ready!"


# --- 3. Start the Frontend GUI ---
echo "Starting Frontend GUI..."
# Start the frontend. The script will wait here until it closes.
$FRONTEND_PATH &
FRONTEND_PID=$!

# Check if the frontend process started successfully
if ! ps -p $FRONTEND_PID > /dev/null; then
    echo "Error: Failed to start frontend at '$FRONTEND_PATH'." >&2
    exit 1
fi

echo "Monitoring frontend process (PID: $FRONTEND_PID)..."
# The 'wait' command pauses script execution until the specified process (the frontend) terminates.
wait $FRONTEND_PID


# --- 4. Automatically Stop the Backend Server ---
# This is now handled by the 'trap cleanup EXIT' function,
# which runs automatically when the script finishes after the 'wait' command completes.
echo "Frontend application has been closed."

# The script will now exit, and the 'trap' will trigger the cleanup function.
echo "Script finished."
