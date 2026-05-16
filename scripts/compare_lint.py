import subprocess
import os
import re
import sys
from collections import defaultdict

def strip_ansi(text):
    ansi_escape = re.compile(r'\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])')
    return ansi_escape.sub('', text)

def run_djlint(path):
    cmd = ["./venv/bin/djlint", "--lint", path]
    result = subprocess.run(cmd, capture_output=True, text=True)
    return parse_djlint_output(result.stdout)

def run_djlintr(path):
    cmd = ["cargo", "run", "--release", "--quiet", "--", "--lint", path]
    result = subprocess.run(cmd, capture_output=True, text=True)
    return parse_djlintr_output(result.stdout)

def parse_djlint_output(output):
    # Example: H025 23:20 Tag seems to be an orphan. <select name="priori
    errors = defaultdict(list)
    current_file = None
    lines = output.splitlines()
    for line in lines:
        line = strip_ansi(line)
        if line.startswith("tests/parity_data/"):
            current_file = line.strip()
        elif re.match(r"^[A-Z]\d{3}\s+\d+:\d+", line.strip()):
            if current_file:
                match = re.match(r"^([A-Z]\d{3})\s+(\d+):(\d+)", line.strip())
                if match:
                    code, l, c = match.groups()
                    errors[current_file].append({
                        "code": code,
                        "line": int(l),
                        "col": int(c),
                        "message": line.strip()
                    })
    return errors

def parse_djlintr_output(output):
    # Example: H014  36: 0 Found extra blank lines.      </tr>
    errors = defaultdict(list)
    current_file = None
    lines = output.splitlines()
    for line in lines:
        line = strip_ansi(line)
        if line.startswith("tests/parity_data/"):
            current_file = line.strip()
        elif re.match(r"^[A-Z]\d{3}\s+\d+:\s*\d+", line.strip()):
            if current_file:
                # Use a more flexible regex for djlintr output
                match = re.match(r"^([A-Z]\d{3})\s+(\d+):\s*(\d+)", line.strip())
                if match:
                    code, l, c = match.groups()
                    errors[current_file].append({
                        "code": code,
                        "line": int(l),
                        "col": int(c),
                        "message": line.strip()
                    })
    return errors

def main():
    data_dir = "tests/parity_data"
    if not os.path.exists(data_dir):
        print(f"Data directory {data_dir} not found. Run 'make fetch-test-data' first.")
        sys.exit(1)

    print(f"Comparing lint results for {data_dir}...")
    
    djlint_errors = run_djlint(data_dir)
    djlintr_errors = run_djlintr(data_dir)
    
    all_files = sorted(set(djlint_errors.keys()) | set(djlintr_errors.keys()))
    
    total_djlint = sum(len(v) for v in djlint_errors.values())
    total_djlintr = sum(len(v) for v in djlintr_errors.values())
    
    print(f"Total errors: djlint={total_djlint}, djlintr={total_djlintr}")
    print("-" * 80)

    discrepancies = 0
    for file in all_files:
        d_errs = djlint_errors.get(file, [])
        dr_errs = djlintr_errors.get(file, [])
        
        d_codes = sorted([e["code"] for e in d_errs])
        dr_codes = sorted([e["code"] for e in dr_errs])
        
        if d_codes != dr_codes:
            discrepancies += 1
            print(f"FILE: {file}")
            
            # Find missing in djlintr
            missing = []
            d_counts = defaultdict(int)
            for c in d_codes: d_counts[c] += 1
            dr_counts = defaultdict(int)
            for c in dr_codes: dr_counts[c] += 1
            
            for code in d_counts:
                if dr_counts[code] < d_counts[code]:
                    missing.append(f"{code} (expected {d_counts[code]}, got {dr_counts[code]})")
            
            # Find extra in djlintr
            extra = []
            for code in dr_counts:
                if d_counts[code] < dr_counts[code]:
                    extra.append(f"{code} (expected {d_counts[code]}, got {dr_counts[code]})")
            
            if missing:
                print(f"  Missing in djlintr: {', '.join(missing)}")
            if extra:
                print(f"  Extra in djlintr:   {', '.join(extra)}")
            print()

    if discrepancies > 0:
        print(f"Found {discrepancies} files with discrepancies.")
        sys.exit(1)
    else:
        print("All files match!")
        sys.exit(0)

if __name__ == "__main__":
    main()
