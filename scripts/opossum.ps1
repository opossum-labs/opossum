# ===================================================================
#             START-AND-MONITOR SCRIPT FOR APPLICATIONS
# ===================================================================
# This script starts a backend server, waits for it to be ready,
# starts a frontend GUI, and then monitors the frontend.
# When the frontend process is closed, it automatically stops the backend.
# ===================================================================

# --- Configuration ---
$OriginalProgressPreference = $Global:ProgressPreference
$Global:ProgressPreference = 'SilentlyContinue'

$backendPath = "C:\Users\ueisenb\AppData\Local\0_gsi_executables\opossum\target\release\opossum_backend.exe"
$frontendPath = "C:\Users\ueisenb\AppData\Local\0_gsi_executables\opossum\target\dx\opossum_gui\release\windows\app\opossum_gui.exe"

# --- Process & Network Configuration ---
# Find process names from Task Manager > Details tab (e.g., "backend-server")
$frontendProcessName = "OPOSSUM GUI"
$backendPort = 8001 # The port your backend listens on for the readiness check

# --- Timeout Configuration ---
$startupTimeoutSeconds = 30 # How long to wait for the backend to start

# ===================================================================
#                           SCRIPT LOGIC
#             (No changes needed below this line)
# ===================================================================

# --- 1. Start the Backend Server ---
try {
    Write-Host "Starting Backend Server: '$backendProcessName'..."
    $backendProcess = Start-Process -FilePath $backendPath -PassThru -WindowStyle Hidden
    Write-Host "Backend process started with ID: $($backendProcess.Id)"
}
catch {
    Write-Error "Failed to start backend at '$backendPath'. Please check the path."
    Read-Host "Press Enter to exit"
    exit 1
}

# --- 2. Wait for the Backend to be Ready ---
Write-Host "Waiting for backend to be available on port $backendPort..."
$elapsed = 0
while ($elapsed -lt $startupTimeoutSeconds) {
    if (Test-NetConnection -ComputerName localhost -Port $backendPort -InformationLevel Quiet -Verbose:$false) {
        Write-Host "Backend is ready!"
        break # Exit the wait loop
    }
    Start-Sleep -Seconds 1
    $elapsed++
}

if ($elapsed -ge $startupTimeoutSeconds) {
    Write-Error "Timeout: Backend did not become available within $startupTimeoutSeconds seconds."
    Write-Host "Stopping backend process..."
    Stop-Process -Id $backendProcess.Id -Force -ErrorAction SilentlyContinue
    Read-Host "Press Enter to exit"
    exit 1
}

# --- 3. Start the Frontend GUI ---
try {
    Write-Host "Starting Frontend GUI: '$frontendProcessName'..."
    $frontendProcess = Start-Process -FilePath $frontendPath -PassThru
}
catch {
    Write-Error "Failed to start frontend at '$frontendPath'. Please check the path."
    Write-Host "Stopping backend process as a precaution..."
    Stop-Process -Id $backendProcess.Id -Force -ErrorAction SilentlyContinue
    Read-Host "Press Enter to exit"
    exit 1
}

# This is the core of the monitoring logic.
# It waits until the process with the given ID is no longer running.
$frontendProcess.WaitForExit()

# --- 5. Automatically Stop the Backend Server ---
Write-Host "Frontend application has been closed."
Write-Host "Stopping the backend server..."

try {
    Stop-Process -Id $backendProcess.Id -Force -ErrorAction Stop
    Write-Host "Backend server has been stopped successfully."
}
catch {
    Write-Warning "Could not stop backend process. It might have already been closed."
}

Write-Host "Script finished."
Start-Sleep -Seconds 1