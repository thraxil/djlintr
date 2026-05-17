import os
import subprocess
import shutil
import difflib
import sys

def main():
    data_dir = "tests/parity_data"
    temp_dir = "temp_reformat"
    
    if not os.path.exists(data_dir):
        print(f"Data directory {data_dir} not found. Run 'make fetch-test-data' first.")
        sys.exit(1)

    if os.path.exists(temp_dir):
        shutil.rmtree(temp_dir)
    os.makedirs(temp_dir)
    
    djlint_dir = os.path.join(temp_dir, "djlint")
    djlintr_dir = os.path.join(temp_dir, "djlintr")
    
    shutil.copytree(data_dir, djlint_dir)
    shutil.copytree(data_dir, djlintr_dir)
    
    print(f"Comparing reformat results for {data_dir}...")
    
    # Common flags for both
    # djlint default max_blank_lines is 0. djlintr default is 1. 
    # Let's force both to 0 for parity testing.
    # Also set profile to django as many tests use it.
    
    print("Running djlint --reformat...")
    djlint_cmd = [
        "./venv/bin/djlint", 
        "--reformat", 
        "--max-blank-lines=0", 
        "--indent=4",
        "--profile=django",
        djlint_dir
    ]
    subprocess.run(djlint_cmd, capture_output=True)
    
    print("Running djlintr --reformat...")
    djlintr_cmd = [
        "cargo", "run", "--release", "--quiet", "--", 
        "--reformat", 
        "--max-blank-lines=0",
        "--profile=django",
        djlintr_dir
    ]
    subprocess.run(djlintr_cmd, capture_output=True)
    
    # Compare
    mismatches = 0
    total_files = 0
    
    # Get all files from djlint_dir to iterate
    all_files = []
    for root, dirs, files in os.walk(djlint_dir):
        for file in files:
            if file.endswith(".html"):
                all_files.append(os.path.relpath(os.path.join(root, file), djlint_dir))
    
    all_files.sort()
    
    for rel_path in all_files:
        total_files += 1
        djlint_file = os.path.join(djlint_dir, rel_path)
        djlintr_file = os.path.join(djlintr_dir, rel_path)
        
        with open(djlint_file, "r", encoding="utf-8", errors="replace") as f:
            djlint_content = f.readlines()
        with open(djlintr_file, "r", encoding="utf-8", errors="replace") as f:
            djlintr_content = f.readlines()
            
        if djlint_content != djlintr_content:
            mismatches += 1
            print(f"\n{'-'*40}")
            print(f"MISMATCH: {rel_path}")
            print(f"{'-'*40}")
            diff = difflib.unified_diff(
                djlint_content, 
                djlintr_content, 
                fromfile="djlint", 
                tofile="djlintr"
            )
            sys.stdout.writelines(diff)
                
    print(f"\n{'-'*80}")
    print(f"Processed {total_files} files.")
    if mismatches > 0:
        print(f"Found {mismatches} files with discrepancies.")
        # shutil.rmtree(temp_dir) # Keep temp_dir for inspection if failed
        sys.exit(1)
    else:
        print("All files match!")
        shutil.rmtree(temp_dir)
        sys.exit(0)

if __name__ == "__main__":
    main()
