import os
import subprocess
import shutil
import difflib
import sys


# Resolve absolute paths to the tool binaries so they work regardless of
# the cwd used when invoking subprocesses.
PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
DJLINT_BIN = os.path.join(PROJECT_ROOT, "venv", "bin", "djlint")
# cargo is invoked via PATH, but we need the project root for --manifest-path
CARGO_MANIFEST = os.path.join(PROJECT_ROOT, "Cargo.toml")


def compare_html_files(djlint_dir, djlintr_dir, label=""):
    """Compare reformatted HTML files between two directories.

    Returns (total_files, mismatches).
    """
    mismatches = 0
    total_files = 0

    all_files = []
    for root, dirs, files in os.walk(djlint_dir):
        for file in files:
            if file.endswith(".html"):
                all_files.append(
                    os.path.relpath(os.path.join(root, file), djlint_dir)
                )

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
            prefix = f"{label}: " if label else ""
            print(f"\n{'-'*40}")
            print(f"MISMATCH: {prefix}{rel_path}")
            print(f"{'-'*40}")
            diff = difflib.unified_diff(
                djlint_content,
                djlintr_content,
                fromfile="djlint",
                tofile="djlintr",
            )
            sys.stdout.writelines(diff)

    return total_files, mismatches


def run_default_tests(temp_dir):
    """Run the default parity tests (CLI flags, no .djlintrc)."""
    data_dir = "tests/parity_data"

    if not os.path.exists(data_dir):
        print(f"Data directory {data_dir} not found. Run 'make fetch-test-data' first.")
        return 0, 0

    djlint_dir = os.path.join(temp_dir, "default", "djlint")
    djlintr_dir = os.path.join(temp_dir, "default", "djlintr")

    shutil.copytree(data_dir, djlint_dir)
    shutil.copytree(data_dir, djlintr_dir)

    print(f"Comparing reformat results for {data_dir}...")

    # Common flags for both.
    # djlint default max_blank_lines is 0, djlintr default is 1.
    # Force both to 0 for parity testing.
    # Also set profile to django as many tests use it.

    print("Running djlint --reformat...")
    subprocess.run(
        [
            DJLINT_BIN,
            "--reformat",
            "--max-blank-lines=0",
            "--indent=4",
            "--profile=django",
            djlint_dir,
        ],
        capture_output=True,
    )

    print("Running djlintr --reformat...")
    subprocess.run(
        [
            "cargo", "run", "--release", "--quiet",
            f"--manifest-path={CARGO_MANIFEST}",
            "--",
            "--reformat",
            "--max-blank-lines=0",
            "--profile=django",
            djlintr_dir,
        ],
        capture_output=True,
    )

    return compare_html_files(djlint_dir, djlintr_dir)


def run_custom_tests(temp_dir):
    """Run per-config parity tests from tests/parity_data_custom/.

    Each subdirectory must contain a .djlintrc and one or more .html files.
    Both tools are run with cwd set to the temp copy so they discover the
    .djlintrc automatically.  No extra CLI flags are passed.
    """
    custom_dir = "tests/parity_data_custom"

    if not os.path.exists(custom_dir):
        return 0, 0

    total_files = 0
    total_mismatches = 0

    subdirs = sorted(
        d
        for d in os.listdir(custom_dir)
        if os.path.isdir(os.path.join(custom_dir, d))
    )

    for subdir in subdirs:
        src = os.path.join(custom_dir, subdir)
        rc_path = os.path.join(src, ".djlintrc")
        if not os.path.exists(rc_path):
            print(f"WARNING: {src} has no .djlintrc, skipping")
            continue

        html_files = [f for f in os.listdir(src) if f.endswith(".html")]
        if not html_files:
            print(f"WARNING: {src} has no .html files, skipping")
            continue

        djlint_work = os.path.join(temp_dir, "custom", subdir, "djlint")
        djlintr_work = os.path.join(temp_dir, "custom", subdir, "djlintr")
        shutil.copytree(src, djlint_work)
        shutil.copytree(src, djlintr_work)

        print(f"Comparing custom config: {subdir}...")

        # Run djlint from the temp copy dir (picks up .djlintrc via cwd).
        # Pass --configuration explicitly to be safe.
        subprocess.run(
            [DJLINT_BIN, "--reformat", "--configuration", ".djlintrc", "."],
            capture_output=True,
            cwd=djlint_work,
        )

        # Run djlintr from the temp copy dir (picks up .djlintrc via cwd).
        subprocess.run(
            [
                "cargo", "run", "--release", "--quiet",
                f"--manifest-path={CARGO_MANIFEST}",
                "--", "--reformat", ".",
            ],
            capture_output=True,
            cwd=djlintr_work,
        )

        files, mismatches = compare_html_files(djlint_work, djlintr_work, label=subdir)
        total_files += files
        total_mismatches += mismatches

    return total_files, total_mismatches


def main():
    temp_dir = "temp_reformat"

    if os.path.exists(temp_dir):
        shutil.rmtree(temp_dir)
    os.makedirs(temp_dir)

    default_files, default_mismatches = run_default_tests(temp_dir)
    custom_files, custom_mismatches = run_custom_tests(temp_dir)

    total_files = default_files + custom_files
    total_mismatches = default_mismatches + custom_mismatches

    print(f"\n{'-'*80}")
    print(f"Processed {total_files} files ({default_files} default, {custom_files} custom).")
    if total_mismatches > 0:
        print(f"Found {total_mismatches} files with discrepancies.")
        sys.exit(1)
    else:
        print("All files match!")
        shutil.rmtree(temp_dir)
        sys.exit(0)


if __name__ == "__main__":
    main()
