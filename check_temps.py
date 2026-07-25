import os
import subprocess
import sys

def get_cpu_temp():
    """Attempts to read CPU temperature from sysfs."""
    paths = [
        "/sys/class/thermal/thermal_zone0/temp",
        "/sys/class/thermal/thermal_zone1/temp",
        "/sys/class/thermal/thermal_zone2/temp",
    ]
    for path in paths:
        if os.path.exists(path):
            try:
                with open(path, "r") as f:
                    temp_raw = f.read().strip()
                    temp = float(temp_raw) / 1000.0
                    return f"{temp:.1f}°C"
            except Exception:
                continue
    return "N/A (Could not read thermal zone)"

def get_gpu_temp():
    """Attempts to get GPU temperature using nvidia-smi."""
    try:
        # Run nvidia-smi to get temperature
        output = subprocess.check_output(["nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader,nounit"], text=True)
        temp = output.strip()
        return f"{temp}°C"
    except Exception:
        return "N/A (nvidia-smi not available or no GPU found)"

if __name__ == "__main__":
    print(f"CPU Temperature: {get_cpu_temp()}")
    print(f"GPU Temperature: {get_gpu_temp()}")
