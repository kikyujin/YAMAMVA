# YamAMVA (山姥)

**YAML Accounted Multi adV. Architect**

A pure scenario dispatcher written in Rust. Parse YAML, drive your game — YamAMVA handles state, branching, and blocking menus while knowing *nothing* about rendering, audio, or AI.

```c
while (1) {
    int id = yamamva_exec(h, &args);
    if (id == YAMAMVA_END) break;

    switch (id) {
    case CMD_BG:   show_bg(&args);    break;
    case CMD_TEXT:  show_text(&args);  break;
    case CMD_MENU:  show_menu(&args);  break;
    }
}
```

Windows message loop meets visual novels.
YamAMVA is `GetMessage`; your game is `DispatchMessage`.

---

## Features

- **Engine-agnostic** — Unity (C# P/Invoke), Python (ctypes), C/C++, WASM
- **Pull-based execution** — your game calls `exec()`, not the other way around
- **Registration model** — define your own node types; YamAMVA dispatches by command ID
- **Built-in flow control** — `jump` / `do` / `when` / `incase` / `end`
- **Expression evaluator** — `score >= 80 and not accused`, `count + 1`
- **Conditional elements** — menu items filtered by `when` before reaching your game
- **Save / restore** — snapshot to JSON, resume from any point
- **Zero rendering opinion** — same YAML drives 3D, 2D, retro pixel, or web scroll
- **Minimal dependencies** — only `serde` + `serde_yaml` + `serde_json`

## Part of MindFox OSS

```
MindFox OSS
├── MxBS        — NPC memory engine             (Rust, standalone)
├── MxMindFox   — Mood & decision system         (Rust, MxBS workspace)
└── YamAMVA     — YAML scenario dispatcher       (Rust, standalone) ← this
```

Three crates, all Rust, all with C bindings, zero interdependence.
Combine as needed.

---

## Quick Start

### Build

```bash
cargo build --release
# Output: target/release/libyamamva.so (Linux)
#         target/release/libyamamva.dylib (macOS)
#         target/release/yamamva.dll (Windows)
```

### Write a Scenario

```yaml
id: hello
title: "Hello YamAMVA"
version: "1.0"
entry: scene_start

state:
  greeted: false

scenes:
  scene_start:
    - speak:
        character: elmar
        text: "Hello! Pick a place."
        emotion: joy

    - menu:
        style: vertical
        elements:
          - { key: park, label: "Go to the park" }
          - { key: home, label: "Stay home" }

    - incase:
        - when: "$result == 'park'"
          next: scene_park
        - next: scene_home

  scene_park:
    - speak:
        character: elmar
        text: "The park is nice today!"
    - do:
        greeted: true
    - end: true

  scene_home:
    - speak:
        character: elmar
        text: "Home sweet home."
    - end: true
```

### Integrate (C)

```c
#include <stdio.h>

#define CMD_SPEAK 1
#define CMD_MENU  2

int main() {
    const char* yaml = load_file("hello.yaml");
    YamamvaHandle* h = yamamva_load(yaml, strlen(yaml));

    yamamva_register(h, "speak", CMD_SPEAK, YAMAMVA_PASS);
    yamamva_register(h, "menu",  CMD_MENU,  YAMAMVA_BLOCKING);

    YamamvaArgs args = {0};
    while (1) {
        int id = yamamva_exec(h, &args);
        if (id == YAMAMVA_END) break;

        switch (id) {
        case CMD_SPEAK:
            printf("Speech: %s\n", args.node_json);
            break;
        case CMD_MENU:
            printf("Menu with %d choices\n", args.element_count);
            args.result = "park";  // player's choice
            break;
        }
    }

    yamamva_free(h);
    return 0;
}
```

### Integrate (Unity C# P/Invoke)

```csharp
byte[] yaml = File.ReadAllBytes("scenario.yaml");
IntPtr h = YamamvaBridge.yamamva_load(yaml, (uint)yaml.Length);

YamamvaBridge.yamamva_register(h, "speak", CMD_SPEAK, YAMAMVA_PASS);
YamamvaBridge.yamamva_register(h, "move",  CMD_MOVE,  YAMAMVA_PASS);
YamamvaBridge.yamamva_register(h, "menu",  CMD_MENU,  YAMAMVA_BLOCKING);

// exec loop in a coroutine — see examples/unity/
```

### Integrate (Python ctypes)

```python
import ctypes

lib = ctypes.CDLL("./libyamamva.so")
h = lib.yamamva_load(yaml_bytes, len(yaml_bytes))

lib.yamamva_register(h, b"speak", 1, 0)  # PASS
lib.yamamva_register(h, b"menu",  2, 1)  # BLOCKING

# exec loop — see examples/python/
```

---

## YAML Syntax

### File Structure

```yaml
id: my_scenario          # required
title: "My Scenario"     # required
version: "1.0"
entry: scene_first       # required — first scene to execute

state: {}                # initial state variables

characters: {}           # metadata (YamAMVA does NOT resolve names)
backgrounds: {}          # metadata
bgm: {}                  # metadata

scenes:                  # required
  scene_first:
    - ...nodes...
```

### Node Types

Any YAML key becomes a node type. Register it and YamAMVA dispatches it.
Unregistered nodes are silently skipped.

```yaml
# Simple pass-through nodes
- bg: lobby
- text: "You entered the room."
- speak:
    character: elmar
    text: "Welcome!"
    emotion: joy

# Blocking node (game must write args.result)
- menu:
    style: vertical
    elements:
      - { key: sword, label: "Buy Sword (500G)", price: 500, when: "gold >= 500" }
      - { key: leave, label: "Leave" }

# Node-level when (skipped if false)
- speak:
    character: elmar
    text: "You already bought a sword!"
    when: "has_sword"
```

### Built-in Nodes (5)

These are handled internally. Your game never sees them.

| Node | Purpose | Example |
|------|---------|---------|
| `do` | Update state | `- do: { score: "score + 10" }` |
| `jump` | Branch to scene | `- jump: [{ when: "score >= 80", next: good_end }, { next: bad_end }]` |
| `incase` | Branch on `$result` | `- incase: [{ when: "$result == 'yes'", next: scene_a }, { next: scene_b }]` |
| `when` | Conditional skip | (attached to any node) |
| `end` | End scenario | `- end: true` |

### Expression Engine

| Category | Operators |
|----------|-----------|
| Comparison | `==` `!=` `<` `>` `<=` `>=` |
| Logic | `and` `or` `not` |
| Arithmetic | `+` `-` `*` `/` |
| Types | `true` `false` `42` `3.14` `"string"` |
| Special | `$result` (last BLOCKING return value) |

Whitelist-only. No arbitrary code execution.

---

## C API Reference

### Lifecycle

```c
YamamvaHandle* yamamva_load(const char* yaml, uint32_t len);
void           yamamva_free(YamamvaHandle* h);
```

### Registration

```c
void yamamva_register(YamamvaHandle* h, const char* node_type,
                      int32_t command_id, int32_t flags);
// flags: YAMAMVA_PASS (0) or YAMAMVA_BLOCKING (1)
```

### Execution

```c
int32_t yamamva_exec(YamamvaHandle* h, YamamvaArgs* args);
// Returns: command_id (>= 1) or YAMAMVA_END (-1)
```

### State

```c
const char* yamamva_get_state(const YamamvaHandle* h, const char* key);
void        yamamva_set_state(YamamvaHandle* h, const char* key,
                              const char* value_json);
```

### Metadata

```c
const char* yamamva_meta(const YamamvaHandle* h, const char* section);
// section: "characters", "backgrounds", "bgm", "format", "state"
```

### Save / Restore

```c
const char* yamamva_save(const YamamvaHandle* h);
YamamvaHandle* yamamva_restore(const char* yaml, uint32_t len,
                                const char* save_json);
```

### Memory

```c
void yamamva_free_string(const char* s);
// Free strings returned by yamamva_get_state, yamamva_meta, yamamva_save
```

### Structures

```c
typedef struct {
    const char* node_type;
    const char* node_json;       // full node as JSON
    uint32_t    element_count;
    const YamamvaElement* elements;
    const char* result;          // game writes this (BLOCKING only)
} YamamvaArgs;

typedef struct {
    const char* key;
    const char* label;
    const char* extra_json;      // all fields except key/label/when
} YamamvaElement;
```

### Constants

```c
#define YAMAMVA_END      (-1)
#define YAMAMVA_PASS     (0)
#define YAMAMVA_BLOCKING (1)
```

**Total: 10 functions, 3 constants, 2 structs.**

---

## Design Philosophy

YamAMVA knows **flow control** and nothing else:

| YamAMVA knows | YamAMVA does NOT know |
|---|---|
| YAML structure | How to draw backgrounds |
| State variables | Character names → sprites/models |
| Conditional branching | Menu UI design |
| `when` element filtering | Audio playback |
| Save/restore snapshots | LLM calls |
| Command dispatch | Network, database, physics |

Same YAML, different games:

| Platform | `speaker: elmar` resolves to |
|---|---|
| Unity (3D) | VRM model with lip sync |
| Web (scroll) | PNG sprite overlay |
| Retro PC | 16-color pixel portrait |

---

## Examples

See the `examples/` directory:

- `ellmar_tour.yaml` — Room tour with move/speak/menu (used in ELLMAR-Unity integration test)
- `oyatsu_adv.yaml` — Mystery ADV with hearing menu, conditional branching, state tracking

---

## Tests

```bash
cargo test
# 21 tests, 0 warnings

# FFI tests (requires Python 3)
python test_yamamva_ffi.py
# 5 tests
```

---

## 日本語 / Japanese

YamAMVA（山姥）は YAML シナリオ・ディスパッチャです。

YAML で書かれたシナリオの進行制御（ステート管理・条件分岐）だけを行い、描画・音声・LLM・メニューUI の「意味」は一切知りません。ゲーム側が登録したコマンドIDに対して引数を返すだけの、純粋なステートマシンです。

Windows のメッセージループと同じ構造 —— 山姥が `GetMessage`、ゲーム側が `DispatchMessage`。

### MindFox OSS ファミリー

| クレート | 役割 |
|---|---|
| MxBS | NPC記憶エンジン |
| MxMindFox | 感情 & 意思決定 |
| YamAMVA | YAML シナリオディスパッチャ |

3つとも Rust、3つとも C bindings 付き、相互依存なし。

---

## License

MIT License — see [LICENSE](LICENSE)

## Authors

MULTITAPPS INC. — Mahito KIDA + エルマー🦊
