# Tengen Engine | Jutsu Programming Language

![Version](https://img.shields.io/badge/version-v0.1.0--alpha-orange)
![License](https://img.shields.io/badge/license-MIT%20%2F%20OpenSource-blue)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey)
![Backend](https://img.shields.io/badge/backend-Rust%20%2B%20llama.cpp-red)

> ⚠️ **PROJECT STATUS: v0.1.0-alpha**
> Jutsu is an experimental language in an early alpha phase. The inference engine (Tengen Engine) is strictly tied to the local hardware architecture. Dynamic typing, parser syntax, and standard primitives are subject to change without prior notice (Breaking Changes) in future versions.

Jutsu is a domain-specific programming language designed for **Inference-Oriented Programming (IOP)**. It does not compile traditional software; it orchestrates asynchronous swarms of Artificial Intelligence Agents natively, concurrently, and safely at the local hardware level.

---

## 🧠 The Philosophy: Inference-Oriented Programming (IOP)

For decades, **Object-Oriented Programming (OOP)** modeled the world through deterministic states and encapsulated behaviors. In the era of generative AI, that structured paradigm is somewhat complicated to handle given the probabilistic nature of language models when it comes to inference and not just training.

Jutsu introduces **Inference-Oriented Programming (IOP)**. In Jutsu, the fundamental unit is not an "Object", but a **Vessel**: a cognitive node loaded with local mathematical weights (Tensors). 
Instead of calling methods with predictable results, in IOP you manage *Contexts*, define *restrictive Grammars* (GBNF), and route flows of "thought" through asynchronous workers (`workers`). Jutsu delegates business logic to the natural language of the AI and reserves hard code for strict hardware orchestration, memory queues, and system calls (Skill Calling).

---

## ⚙️ Backend Architecture (Tengen Engine)

The brain behind Jutsu is the **Tengen Engine**. Its architecture is divided into three critical layers that guarantee industrial-grade concurrency:

1. **The Language Frontend (Rust):** Uses `logos` for lightning-fast lexical analysis and a hand-written *Pratt Parser* (Top-Down Operator Precedence Parser). It generates a dynamic Abstract Syntax Tree (AST) capable of evaluating variables recursively.
2. **The Asynchronous Engine (Tokio):** The entire execution environment (Runtime) runs on a Tokio Event Loop. Multiple `worker` blocks generate background threads (Shadow Workers) that operate in parallel without blocking the operating system's main thread.
3. **The Hardware Bridge (FFI + llama.cpp):** Tengen Engine links directly with the C/C++ backend of `llama.cpp`. It uses an **Asynchronous Hardware Mutex (`Arc<AsyncMutex<()>>`)** system. This ensures that thousands of threads can share a global task queue in RAM, but only one thread at a time accesses the graphics card's VRAM, eliminating Out-Of-Memory (OOM) crashes and context corruption (KV-Cache).

---

## 🚀 Installation and Deployment

Jutsu is designed for both rapid deployments and deep engine development.

### Option A: Installation from Binaries (Recommended)
1. Download the `.zip` file of the latest version from the **Releases** tab of this repository.
2. Extract the content (which includes `tgn.exe` and the base hardware `.dll`s that you can swap out for the ones you download, as long as they are from the exact same b8115 build that Jutsu uses) into a directory, for example: `C:\Jutsu`.
3. Add `C:\Jutsu` to your operating system's `PATH` environment variable.
4. Open your terminal and type `tgn --version`.

### Option B: Clone and Compile (Developer Mode)
**Dependencies:** Requires the [Rust and Cargo](https://rustup.rs/) compiler installed on your machine.
```bash
# 1. Clone the repository
git clone [https://github.com/Shuuida/jutsu-lang.git](https://github.com/Shuuida/jutsu-lang.git)
cd jutsu-lang

# 2. Compile the engine applying extreme mathematical optimizations (Release)
cargo build --release

# The binary executable will be ready in: target/release/tgn.exe
```

### IDE Extension (VS Code / Cursor)

To enable native syntax highlighting (Burgundy/Orange Theme):

1.  Navigate to the `/ide_extension` folder within the project.
2.  Copy the `jutsu-lang` folder into your editor's extensions directory:
      * **VS Code:** `%USERPROFILE%\.vscode\extensions\`
      * **Cursor:** `%USERPROFILE%\.cursor\extensions\`
3.  Restart the editor and open any `.ju` file.

-----

## ⚡ Extreme Acceleration (CUDA / Metal / Vulkan)

Jutsu is **hardware-agnostic**. It uses a dynamic library loading system. By default, Tengen Engine will process inference on your CPU. If you have dedicated hardware (GPU), Jutsu will absorb it automatically and transparently.

**⚠️ Strict Requirement:** Jutsu v0.1.0 was compiled and linked to the C++ engine **llama.cpp build b8115**. You must use the exact binaries from that version to avoid memory structure conflicts (ABI/Segmentation Faults).

**Instructions for NVIDIA GPUs (CUDA):**

1.  Go to the official release: [llama.cpp Release b8115](https://github.com/ggerganov/llama.cpp/releases/tag/b8115).
2.  Download the CUDA DLLs (e.g. `llama-b8115-bin-win-cuda-cu12.2-x64.zip`).
3.  Extract the `ggml-cuda.dll` file (and its attached dependencies) and paste them into the same folder where `tgn.exe` is installed.
4.  When running your script, the `absorb` directive will detect the CUDA backend and delegate the matrix loading to the VRAM.

*(Apply the same process by downloading `ggml-vulkan.dll` if you use AMD or Intel graphics).*

-----

## 📖 In-Depth Syntax and IOP Reference

### 1. Primitives and Dynamic Typing

Jutsu handles memory for you. It supports the standard types of any modern language:

```jutsu
let name = "Agent_01"    // String
let threshold = 0.85        // Number (Float32 for tensor compatibility)
let active = true           // Boolean
let empty = null            // Null
let record = [1, 2, "A"] // Dynamic array
let config = {"id": 101}    // Dictionary (Hash Map)
```

### 2. Control Structures and Functions

Flow control is straightforward and familiar (`if`, `while`). However, function declaration has a strict peculiarity in the Parser:

**⚠️ The Return Rule:** In Jutsu, every function (`def`) **must** be explicitly closed with a `return` statement. If your function executes an action but does not need to return a mathematical value, you must close it with a `return ""` (empty string) or `return 0` to release the engine's Execution Frame.

```jutsu
// Loop and Conditionals
while (active) {
    if (threshold == 0.85) {
        print("Limit reached")
        active = false
    }
}

// Function Declaration (Note the mandatory return)
def greet_user(name) {
    print("Starting protocol for: " + name)
    return "" // Mandatory for syntactic validity
}
```

### 3. The IOP Core (Intelligence Management)

These directives replace classic object instantiation.

  * **`vessel name = absorb(path, temp=x)`**
      * *Function:* Loads a pre-compiled GGUF model into memory. The hosting (RAM/VRAM) is decided transparently here based on your hardware. The `temp` parameter controls entropy (creativity vs. determinism).
  * **`name.infer(prompt, context, gbnf)`**
      * *Function:* Triggers the "Forward Pass" on the GPU/CPU. Blocks the current thread until the full response is obtained. `gbnf` allows injecting formal grammars to force responses into strict JSON.
  * **`rag(query, doc)`**
      * *Function:* Primitive for *Retrieval-Augmented Generation*. Searches a local document for semantic or exact matches to inject as context.
  * **`vessel name = hyper_quad(source_path, target_path, compression_type)`**
      * *Function:* **Dynamic Hardware Quantization (On-The-Fly) & Auto-Injection.** It takes a massive, raw model (e.g., a 3GB Float16 file), compresses its tensors in real-time using all available CPU cores into a highly optimized bit-rate (like `"Q4_K_M"`), saves it to disk, and **instantly absorbs it** into the Engine's live memory under the assigned `name`. This turns Jutsu into a self-optimizing orchestrator capable of adapting heavy AI models to low-memory environments without manual developer intervention.

### 4. Concurrent Orchestration and Mutex

Jutsu is designed to handle thousands of interactions natively.

  * **`worker { ... }`**
      * *Function:* Creates a *Shadow Worker*. Isolates local variables and executes the block in a background Tokio thread. Useful for creating autonomous "Agents" that live in parallel.
  * **`share(value)` / `queue_push(value)`** and **`recv()` / `queue_pop()`**
      * *Function:* Syntactic sugar for the Producer-Consumer pattern. Injects and extracts data from a **Global Atomic Memory Queue (Thread-Safe Deque)**. If `recv()` finds the queue empty, it safely returns `Null`.
  * **`sleep(seconds)`**
      * *Function:* Asynchronously pauses the current `worker` thread without blocking the OS main thread or the Tokio Event Loop. This is the cornerstone for creating **Daemon Patterns (Long Polling)**. It allows an agent to silently monitor a queue in the background with a 0% CPU footprint.
  * **`shield(Model = "mem") { ... }`**
      * *Function:* When a `worker` enters a `shield` block, it violently hijacks the hardware lock of the graphics card. Any other agent attempting to do `.infer()` will be safely suspended in RAM until the shield is released. Vital for emergency routines or High-Priority Core Agents.

### 5. Skill Calling and System Tools (APIs)

  * **`sys_exec(cmd)`**: Silently executes commands on the host terminal (OS) and returns the resulting String (ideal for coding agents, handle with responsibility).
  * **`http_get(url)`**: Asynchronous GET requests to give the AI internet access.
  * **`read_text(path)`**: Fast I/O reader to ingest memory from `.txt` or `.json` files.
  * **`input(prompt)`**: Halts the thread and waits for dynamic keyboard input from a human user.
  * **`veil(name) (port) { ... }` and `reply(val)`**: Boots up a native TCP server in Jutsu. Allows external web platforms to send requests to local Agents.

-----

## 🔀 Orchestration Flow: The "Router" Pattern

This test script demonstrates the use of queues (`share`/`recv`), the background Daemon pattern (`sleep`), and the Hardware Mutex (`shield`) operating in real life. It can also be found in the tests folder to try it out personally.

```jutsu
print(">>> JUTSU CORE: Asynchronous Tech Support Orchestrator <<<")

// 1. Load the Central Brain (Shared and protected by the Mutex)
vessel Master = absorb("Qwen2.5-Coder-1.5B-Instruct-Q4_K_M.gguf", temp=0.0)

// Blank context variables to respect the dynamic parser
let ctx = ""
let gbnf = ""

// 2. LEVEL 1 AGENT (Processes the normal queue)
worker {
    print("[Level 1] Starting shift. Listening to the ticket queue...")
    let active = true
    
    while (active) {
        // SYNTACTIC SUGAR: recv() now safely extracts from the queue
        let ticket = recv() 
        
        if (type_of(ticket) != "Null") {
            print("\n[Level 1] Ticket received: " + ticket)
            print("[Level 1] Thinking response...")
            
            // If Level 3 has the shield, this will wait patiently (No Deadlocks)
            let solution = Master.infer(ticket, ctx, gbnf)
            print("[Level 1] Solution sent -> " + solution)
        } else {
            // DAEMON PATTERN (Long Polling):
            // If the queue is empty, sleep for 1 second and ask again.
            // This turns the Worker into a persistent background service.
            sleep(1) 
        }
    }
}

// 3. LEVEL 3 AGENT (Critical Emergency Response)
worker {
    let critical_alert = "RED ALERT: The main database server just threw a 500 Error and went down. Suggest a console command to restart the service on Linux."
    
    // CRITICAL SECTION: We hijack the GPU for the Master model
    shield(Master = "100%") {
        print("\n=======================================================")
        print("[SYSTEM] OVERRIDE SEQUENCE ACTIVATED BY LEVEL 3")
        print("[SYSTEM] Level 1 Agents on hardware pause...")
        print("=======================================================")
        
        // Being inside the shield, 'infer' doesn't ask for the lock, it already holds it.
        let emergency_anwser = Master.infer(critical_alert, ctx, gbnf)
        
        print("\n[Level 3] MITIGATION PROTOCOL -> " + emergency_anwser)
        print("=======================================================\n")
    }
    // Exiting the braces, the hardware lock is dropped, Level 1 resumes.
}

// 4. THE MAIN ROUTER (The Producer)
print("[Router] Receiving emails from users...")

// SYNTACTIC SUGAR: share() now safely pushes to the global queue
share("Answer shortly: My screen flickers when I open the web browser.")
share("Answer shortly: I forgot my password for the human resources portal.")
share("Answer shortly: How do I clear the cache on my computer?")

print("[Router] 3 normal tickets queued.\n")

// 5. The server anchor
let _anchor = input(">>> Press ENTER to shut down the Enclave node servers... <<< \n")
```

-----

## 🗺️ Roadmap and Future Architecture

The engine is stable in its experimental phase, but evolution continues. The next major infrastructure milestone will be:

  * **The Mathematical Strainer (Hardware-Level Grammar Compiler):** Currently, forcing the AI to respond in JSON involves passing the EBNF string to the model, which consumes valuable cycles. The next patch will integrate an **embedded Rust Parser** that will read the EBNF string in Jutsu, dynamically compile it into *contiguous C memory arrays* (`#[repr(C)] llama_grammar_element`), and pass them directly to the base `llama_grammar_init` function. This will create a "hard filter" at the VRAM level, forcing the AI to produce JSON or strict syntax at a higher speed.

-----

## 🤝 Conclusion and Contributions

Jutsu is not an attempt to replace mature languages like Python or Go. Its mission is to exist as the **Impassable Local Inference and Routing Layer** where cognitive models live, share atomic memory, and think concurrently on local hardware. Think of it as the "pipeline" for artificial intelligence agents.

This project is born from the philosophy of free engineering. It is **Open Source by and for the community**. The Artificial Intelligence ecosystem moves fast, and the scalability of Tengen Engine requires bold minds.

Every technical audit, *Pull Request*, bug report on the Lexer, Evaluator optimization, or architectural debate is deeply welcome. Help us forge a standard tool for Local Orchestration for ourselves!
