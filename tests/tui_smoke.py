#!/usr/bin/env python3
"""Tier 1 PTY Integration Test — drives Pi Hybrid TUI via pseudo-terminal.

Run from the Air or mini:
    python3 tests/tui_smoke.py

Prerequisites:
    pip3 install pexpect
    cargo build   (in rust-core/)
"""

import pexpect
import sys
import time

BINARY = "./target/debug/rust-core"
COLS = 120
ROWS = 40


def spawn_pi():
    """Spawn Pi in a PTY with known dimensions."""
    child = pexpect.spawn(BINARY, dimensions=(ROWS, COLS), timeout=5, encoding="utf-8")
    # Give the TUI a moment to draw
    time.sleep(1.5)
    return child


def assert_screen(child, needle, label):
    """Check that the rendered screen contains a string."""
    # Read whatever is on screen
    try:
        child.expect(needle, timeout=3)
        print(f"  PASS: {label}")
        return True
    except pexpect.TIMEOUT:
        print(f"  FAIL: {label} — '{needle}' not found on screen")
        return False


def send_key(child, key, label):
    """Send a key and check the app is still alive."""
    try:
        child.send(key)
        time.sleep(0.3)
        assert child.isalive(), f"App died after {label}"
        return True
    except Exception as e:
        print(f"  FAIL: {label} — {e}")
        return False


def main():
    print("=== Pi Hybrid TUI Smoke Test ===\n")
    failures = 0

    # ─── 1. Startup ───────────────────────────────────────
    print("[1] Startup")
    child = spawn_pi()
    assert child.isalive(), "Failed to spawn Pi"

    # Should see the app title in the status bar
    if assert_screen(child, "Pi Hybrid", "app title visible"):
        pass
    else:
        failures += 1

    # ─── 2. Tab through panes ─────────────────────────────
    print("\n[2] Pane switching")

    panes = ["Files", "Editor", "Agents", "Plan"]
    for pane in panes:
        if not send_key(child, "\t", f"Tab → {pane}"):
            failures += 1

    # After 4 tabs we should be back at Files
    for _ in range(4):
        child.send("\t")
        time.sleep(0.2)

    # ─── 3. Command palette ───────────────────────────────
    print("\n[3] Command palette")
    if send_key(child, "\x10", "Ctrl+P open palette"):  # Ctrl+P
        if assert_screen(child, "Command", "palette visible"):
            pass
        else:
            failures += 1
        child.send("\x1b")  # Esc to close
        time.sleep(0.3)

    # ─── 4. Help popup ────────────────────────────────────
    print("\n[4] Help popup (F1)")
    # F1 is escape sequence in some terminals
    child.send("\x1bOP")  # F1
    time.sleep(0.5)
    if assert_screen(child, "Navigation", "help shows Navigation"):
        pass
    else:
        failures += 1
    child.send("\x1b")  # Esc to close
    time.sleep(0.3)

    # ─── 5. Toggle dark mode (F7) ─────────────────────────
    print("\n[5] Dark mode toggle (F7)")
    if send_key(child, "\x1b[18~", "F7 toggle dark mode"):
        pass
    else:
        failures += 1

    # ─── 6. Resize terminal ───────────────────────────────
    print("\n[6] Terminal resize")
    try:
        child.setwinsize(60, 24)
        time.sleep(0.5)
        assert child.isalive(), "Died on resize to 60x24"
        child.setwinsize(200, 60)
        time.sleep(0.5)
        assert child.isalive(), "Died on resize to 200x60"
        child.setwinsize(ROWS, COLS)
        time.sleep(0.5)
        print("  PASS: resize survived")
    except Exception as e:
        print(f"  FAIL: resize — {e}")
        failures += 1

    # ─── 7. Rapid keystrokes (stress) ─────────────────────
    print("\n[7] Rapid keystroke stress (200 keys)")
    try:
        for _ in range(50):
            child.send("jkkkllhhgg\t")
        time.sleep(1.0)
        assert child.isalive(), "Died on rapid keystrokes"
        print("  PASS: rapid keystrokes survived")
    except Exception as e:
        print(f"  FAIL: keystroke stress — {e}")
        failures += 1

    # ─── 8. Graceful quit ─────────────────────────────────
    print("\n[8] Graceful quit (q)")
    try:
        child.send("q")
        time.sleep(0.5)
        child.expect(pexpect.EOF, timeout=3)
        print("  PASS: clean exit with 'q'")
    except Exception as e:
        print(f"  FAIL: quit — {e}")
        failures += 1

    # ─── Report ───────────────────────────────────────────
    print(f"\n{'='*50}")
    if failures == 0:
        print("ALL 8 TESTS PASSED — Pi Hybrid is boring!")
    else:
        print(f"{failures} TEST(S) FAILED — see above")
    print(f"{'='*50}")
    return failures


if __name__ == "__main__":
    sys.exit(main())
