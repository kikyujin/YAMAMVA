import ctypes
import json
import os
import sys

LIB_PATH = os.path.join(os.path.dirname(__file__), "target", "release", "libyamamva.so")
if not os.path.exists(LIB_PATH):
    dylib = LIB_PATH.replace(".so", ".dylib")
    if os.path.exists(dylib):
        LIB_PATH = dylib

lib = ctypes.cdll.LoadLibrary(LIB_PATH)


class FfiElement(ctypes.Structure):
    _fields_ = [
        ("key", ctypes.c_char_p),
        ("label", ctypes.c_char_p),
        ("extra_json", ctypes.c_char_p),
    ]


class FfiArgs(ctypes.Structure):
    _fields_ = [
        ("node_type", ctypes.c_char_p),
        ("node_json", ctypes.c_char_p),
        ("element_count", ctypes.c_uint32),
        ("elements", ctypes.POINTER(FfiElement)),
        ("result", ctypes.c_char_p),
    ]


lib.yamamva_load.argtypes = [ctypes.c_char_p, ctypes.c_uint32]
lib.yamamva_load.restype = ctypes.c_void_p

lib.yamamva_register.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32]
lib.yamamva_register.restype = None

lib.yamamva_exec.argtypes = [ctypes.c_void_p, ctypes.POINTER(FfiArgs)]
lib.yamamva_exec.restype = ctypes.c_int32

lib.yamamva_get_state.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
lib.yamamva_get_state.restype = ctypes.c_void_p

lib.yamamva_meta.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
lib.yamamva_meta.restype = ctypes.c_void_p

lib.yamamva_save.argtypes = [ctypes.c_void_p]
lib.yamamva_save.restype = ctypes.c_void_p

lib.yamamva_free.argtypes = [ctypes.c_void_p]
lib.yamamva_free.restype = None

lib.yamamva_free_string.argtypes = [ctypes.c_void_p]
lib.yamamva_free_string.restype = None

YAMAMVA_END = -1
YAMAMVA_PASS = 0
YAMAMVA_BLOCKING = 1

CMD_BG = 1
CMD_TEXT = 2
CMD_SPEAKER = 3
CMD_MENU = 4
CMD_MXBS = 5


def read_and_free(ptr):
    if not ptr:
        return None
    s = ctypes.cast(ptr, ctypes.c_char_p).value.decode("utf-8")
    lib.yamamva_free_string(ptr)
    return s


# ─── Test 1: Simple linear scenario ───
print("=== Test 1: Linear flow ===")

yaml1 = b"""
id: test_linear
title: Linear Test
entry: scene_start
scenes:
  scene_start:
    - bg: lobby
    - text: "Hello world"
    - speaker: elmar
      text: "Hi!"
    - end: true
"""

h = lib.yamamva_load(yaml1, len(yaml1))
assert h, "load failed"

lib.yamamva_register(h, b"bg", CMD_BG, YAMAMVA_PASS)
lib.yamamva_register(h, b"text", CMD_TEXT, YAMAMVA_PASS)
lib.yamamva_register(h, b"speaker", CMD_SPEAKER, YAMAMVA_PASS)

args = FfiArgs()
results = []

for _ in range(10):
    cmd = lib.yamamva_exec(h, ctypes.byref(args))
    if cmd == YAMAMVA_END:
        results.append("END")
        break
    nt = args.node_type.decode("utf-8") if args.node_type else "?"
    results.append((cmd, nt))

assert results == [(CMD_BG, "bg"), (CMD_TEXT, "text"), (CMD_SPEAKER, "speaker"), "END"]
print(f"  OK: {results}")

lib.yamamva_free(h)


# ─── Test 2: Blocking + incase ───
print("\n=== Test 2: Blocking + incase ===")

yaml2 = b"""
id: test_blocking
title: Blocking Test
entry: scene_start
state:
  heard: false
scenes:
  scene_start:
    - bg: lobby
    - hearingmenu:
        style: vertical
        elements:
          - { key: elmar, label: "Go Elmar" }
          - { key: leave, label: "Leave" }
    - incase:
        - when: "$result == 'elmar'"
          do: { heard: true }
          next: scene_elmar
        - next: scene_bye
  scene_elmar:
    - text: "Elmar says hi"
    - end: true
  scene_bye:
    - text: "Goodbye"
    - end: true
"""

h = lib.yamamva_load(yaml2, len(yaml2))
assert h, "load failed"

lib.yamamva_register(h, b"bg", CMD_BG, YAMAMVA_PASS)
lib.yamamva_register(h, b"text", CMD_TEXT, YAMAMVA_PASS)
lib.yamamva_register(h, b"hearingmenu", CMD_MENU, YAMAMVA_BLOCKING)

args = FfiArgs()

# Step 1: bg
cmd = lib.yamamva_exec(h, ctypes.byref(args))
assert cmd == CMD_BG
print(f"  Step 1: cmd={cmd} type={args.node_type.decode()}")

# Step 2: hearingmenu (BLOCKING)
cmd = lib.yamamva_exec(h, ctypes.byref(args))
assert cmd == CMD_MENU
assert args.element_count == 2
el0_key = args.elements[0].key.decode("utf-8")
el1_key = args.elements[1].key.decode("utf-8")
print(f"  Step 2: cmd={cmd} type={args.node_type.decode()} elements=[{el0_key}, {el1_key}]")

# Player chooses "elmar" - keep reference alive
result_buf = ctypes.c_char_p(b"elmar")
args.result = result_buf

# Step 3: incase processes internally, lands on scene_elmar text
cmd = lib.yamamva_exec(h, ctypes.byref(args))
assert cmd == CMD_TEXT
nj = json.loads(args.node_json.decode("utf-8"))
assert "Elmar says hi" in nj.get("text", "")
print(f"  Step 3: cmd={cmd} text={nj['text']}")

# Step 4: end
cmd = lib.yamamva_exec(h, ctypes.byref(args))
assert cmd == YAMAMVA_END
print(f"  Step 4: END")

# Check state
ptr = lib.yamamva_get_state(h, b"heard")
val = read_and_free(ptr)
assert val == "true", f"expected 'true', got '{val}'"
print(f"  State heard = {val}")

lib.yamamva_free(h)


# ─── Test 3: Save / Restore ───
print("\n=== Test 3: Save / Restore ===")

yaml3 = b"""
id: test_save
title: Save Test
entry: scene_start
state:
  counter: 0
scenes:
  scene_start:
    - bg: lobby
    - do:
        counter: "counter + 1"
    - text: "After do"
    - text: "Final text"
    - end: true
"""

h = lib.yamamva_load(yaml3, len(yaml3))
assert h, "load failed"

lib.yamamva_register(h, b"bg", CMD_BG, YAMAMVA_PASS)
lib.yamamva_register(h, b"text", CMD_TEXT, YAMAMVA_PASS)

args = FfiArgs()

# bg
cmd = lib.yamamva_exec(h, ctypes.byref(args))
assert cmd == CMD_BG

# do is consumed internally, "After do" returned
cmd = lib.yamamva_exec(h, ctypes.byref(args))
assert cmd == CMD_TEXT

# Save here
save_ptr = lib.yamamva_save(h)
save_json = read_and_free(save_ptr)
save_data = json.loads(save_json)
print(f"  Saved: counter={save_data['state']['counter']}, scene={save_data['scene_id']}")
assert save_data["state"]["counter"] == 1

lib.yamamva_free(h)

# Restore
save_bytes = save_json.encode("utf-8")
h2 = lib.yamamva_restore(yaml3, len(yaml3), save_bytes)
assert h2, "restore failed"

lib.yamamva_register(h2, b"bg", CMD_BG, YAMAMVA_PASS)
lib.yamamva_register(h2, b"text", CMD_TEXT, YAMAMVA_PASS)

args2 = FfiArgs()
cmd = lib.yamamva_exec(h2, ctypes.byref(args2))
assert cmd == CMD_TEXT
nj = json.loads(args2.node_json.decode("utf-8"))
assert "Final text" in nj.get("text", "")
print(f"  Restored, next text: {nj['text']}")

cmd = lib.yamamva_exec(h2, ctypes.byref(args2))
assert cmd == YAMAMVA_END
print(f"  END after restore")

lib.yamamva_free(h2)


# ─── Test 4: Meta API ───
print("\n=== Test 4: Meta API ===")

h = lib.yamamva_load(yaml2, len(yaml2))
assert h, "load failed"

ptr = lib.yamamva_meta(h, b"state")
state_json = read_and_free(ptr)
state = json.loads(state_json)
print(f"  Initial state: {state}")
assert state.get("heard") == False

lib.yamamva_free(h)


# ─── Test 5: Oyatsu ADV (full scenario) ───
print("\n=== Test 5: Oyatsu ADV full scenario ===")

yaml_path = os.path.join(os.path.dirname(__file__), "..", "docs", "oyatsu_adv.yaml")
with open(yaml_path, "r") as f:
    yaml_full = f.read().encode("utf-8")

h = lib.yamamva_load(yaml_full, len(yaml_full))
assert h, "load failed"

lib.yamamva_register(h, b"bg", CMD_BG, YAMAMVA_PASS)
lib.yamamva_register(h, b"text", CMD_TEXT, YAMAMVA_PASS)
lib.yamamva_register(h, b"speaker", CMD_SPEAKER, YAMAMVA_PASS)
lib.yamamva_register(h, b"choice", CMD_MENU, YAMAMVA_BLOCKING)
lib.yamamva_register(h, b"mxbs_push", CMD_MXBS, YAMAMVA_PASS)

args = FfiArgs()
trace = []
choices_made = []

# Script: intro → hearing_menu → choose Elmar → back to menu → accuse → choose Til → win
choice_script = [
    "scene_hear_elmar",   # First choice: go to Elmar
    "scene_accuse",       # Second choice: accuse
    "scene_judge",        # Third choice: Til (accused: til via do)
]
choice_idx = 0

for step in range(200):
    cmd = lib.yamamva_exec(h, ctypes.byref(args))

    if cmd == YAMAMVA_END:
        trace.append("END")
        break

    nt = args.node_type.decode("utf-8") if args.node_type else "?"
    trace.append(nt)

    if cmd == CMD_MENU and args.element_count > 0:
        el_keys = []
        el_extras = []
        for i in range(args.element_count):
            k = args.elements[i].key.decode("utf-8") if args.elements[i].key else ""
            extra_str = args.elements[i].extra_json.decode("utf-8") if args.elements[i].extra_json else "{}"
            el_keys.append(k)
            el_extras.append(json.loads(extra_str))

        # choice nodes have "next" in extra, pick from script
        chosen = None
        if choice_idx < len(choice_script):
            target = choice_script[choice_idx]
            for i, extra in enumerate(el_extras):
                if extra.get("next") == target:
                    chosen = el_keys[i] if el_keys[i] else None
                    # For choice nodes the "text" is the label, key might be empty
                    # Need to set result to match what incase/choice expects
                    break

        if chosen is None and el_keys:
            chosen = el_keys[0]

        if chosen:
            choices_made.append(chosen)
            # For choice nodes, the result might need to be the text or key
            # Actually for choice nodes in MxADVeng, there's no incase - the next is in the option itself
            # The engine handles choice by... let's check what the parser does
            result_bytes = chosen.encode("utf-8")
            result_buf = ctypes.c_char_p(result_bytes)
            args.result = result_buf

        choice_idx += 1

node_types = [t for t in trace if t != "END"]
print(f"  Total nodes dispatched: {len(node_types)}")
print(f"  Node types: {set(node_types)}")
print(f"  Choices made: {choices_made}")
print(f"  Ended: {'END' in trace}")

assert "END" in trace, "Scenario did not reach END"

# Check final state
ptr = lib.yamamva_get_state(h, b"heard_elmar")
val = read_and_free(ptr)
print(f"  heard_elmar = {val}")

ptr = lib.yamamva_get_state(h, b"accused")
val = read_and_free(ptr)
print(f"  accused = {val}")

lib.yamamva_free(h)


print("\n" + "=" * 40)
print("=== ALL FFI TESTS PASSED ===")
print("=" * 40)
