# Graph Report - my-pi-hybrid  (2026-06-20)

## Corpus Check
- 101 files · ~79,741 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1838 nodes · 3532 edges · 88 communities (76 shown, 12 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 20 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `af202bce`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 39|Community 39]]
- [[_COMMUNITY_Community 40|Community 40]]
- [[_COMMUNITY_Community 41|Community 41]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 43|Community 43]]
- [[_COMMUNITY_Community 44|Community 44]]
- [[_COMMUNITY_Community 45|Community 45]]
- [[_COMMUNITY_Community 46|Community 46]]
- [[_COMMUNITY_Community 47|Community 47]]
- [[_COMMUNITY_Community 48|Community 48]]
- [[_COMMUNITY_Community 49|Community 49]]
- [[_COMMUNITY_Community 50|Community 50]]
- [[_COMMUNITY_Community 51|Community 51]]
- [[_COMMUNITY_Community 52|Community 52]]
- [[_COMMUNITY_Community 53|Community 53]]
- [[_COMMUNITY_Community 54|Community 54]]
- [[_COMMUNITY_Community 60|Community 60]]
- [[_COMMUNITY_Community 61|Community 61]]
- [[_COMMUNITY_Community 62|Community 62]]
- [[_COMMUNITY_Community 63|Community 63]]
- [[_COMMUNITY_Community 64|Community 64]]
- [[_COMMUNITY_Community 65|Community 65]]
- [[_COMMUNITY_Community 66|Community 66]]
- [[_COMMUNITY_Community 67|Community 67]]
- [[_COMMUNITY_Community 68|Community 68]]
- [[_COMMUNITY_Community 69|Community 69]]
- [[_COMMUNITY_Community 70|Community 70]]
- [[_COMMUNITY_Community 71|Community 71]]
- [[_COMMUNITY_Community 72|Community 72]]
- [[_COMMUNITY_Community 73|Community 73]]
- [[_COMMUNITY_Community 74|Community 74]]
- [[_COMMUNITY_Community 75|Community 75]]
- [[_COMMUNITY_Community 76|Community 76]]
- [[_COMMUNITY_Community 77|Community 77]]
- [[_COMMUNITY_Community 78|Community 78]]
- [[_COMMUNITY_Community 79|Community 79]]
- [[_COMMUNITY_Community 80|Community 80]]
- [[_COMMUNITY_Community 81|Community 81]]
- [[_COMMUNITY_Community 82|Community 82]]
- [[_COMMUNITY_Community 83|Community 83]]
- [[_COMMUNITY_Community 84|Community 84]]
- [[_COMMUNITY_Community 85|Community 85]]
- [[_COMMUNITY_Community 86|Community 86]]

## God Nodes (most connected - your core abstractions)
1. `App` - 46 edges
2. `config_with_defaults()` - 31 edges
3. `render_to_string()` - 28 edges
4. `PluginRegistry` - 26 edges
5. `GitManager` - 24 edges
6. `Agent` - 23 edges
7. `render_pane()` - 23 edges
8. `BridgeClient` - 21 edges
9. `PlanManager` - 21 edges
10. `ProviderRegistry` - 21 edges

## Surprising Connections (you probably didn't know these)
- `run_pi_direct()` --calls--> `forward_to_pi()`  [INFERRED]
  scripts/needle_bench.py → scripts/needle_bridge_adapter.py
- `print_live_usage_summary()` --calls--> `load_stats()`  [INFERRED]
  scripts/needle_bench.py → scripts/needle_routing_stats.py
- `print_live_usage_summary()` --calls--> `stats_path()`  [INFERRED]
  scripts/needle_bench.py → scripts/needle_routing_stats.py
- `print_live_usage_summary()` --calls--> `summarize_stats()`  [INFERRED]
  scripts/needle_bench.py → scripts/needle_routing_stats.py
- `main()` --calls--> `workspace_root()`  [INFERRED]
  scripts/needle_bench.py → scripts/needle_bridge_adapter.py

## Import Cycles
- 1-file cycle: `rust-core/benches/bench_main.rs -> rust-core/benches/bench_main.rs`
- 1-file cycle: `rust-core/src/main.rs -> rust-core/src/main.rs`
- 1-file cycle: `rust-core/src/agent/agent_core.rs -> rust-core/src/agent/agent_core.rs`
- 1-file cycle: `rust-core/src/agent/bridge_client.rs -> rust-core/src/agent/bridge_client.rs`
- 1-file cycle: `rust-core/src/agent/compaction.rs -> rust-core/src/agent/compaction.rs`
- 1-file cycle: `rust-core/src/agent/git.rs -> rust-core/src/agent/git.rs`
- 1-file cycle: `rust-core/src/agent/message.rs -> rust-core/src/agent/message.rs`
- 1-file cycle: `rust-core/src/agent/mod.rs -> rust-core/src/agent/mod.rs`
- 1-file cycle: `rust-core/src/agent/plugins.rs -> rust-core/src/agent/plugins.rs`
- 1-file cycle: `rust-core/src/agent/subagents.rs -> rust-core/src/agent/subagents.rs`
- 1-file cycle: `rust-core/src/headless.rs -> rust-core/src/headless.rs`
- 1-file cycle: `rust-core/src/agent/providers.rs -> rust-core/src/agent/providers.rs`
- 1-file cycle: `rust-core/src/agent/subagent.rs -> rust-core/src/agent/subagent.rs`
- 1-file cycle: `rust-core/src/bridge/json_rpc.rs -> rust-core/src/bridge/json_rpc.rs`
- 1-file cycle: `rust-core/src/config.rs -> rust-core/src/config.rs`
- 1-file cycle: `rust-core/src/keybindings.rs -> rust-core/src/keybindings.rs`
- 1-file cycle: `rust-core/src/tui/plan_pane.rs -> rust-core/src/tui/plan_pane.rs`
- 1-file cycle: `rust-core/src/session/store.rs -> rust-core/src/session/store.rs`
- 1-file cycle: `rust-core/src/shutdown.rs -> rust-core/src/shutdown.rs`
- 1-file cycle: `rust-core/src/tui/agent_pane.rs -> rust-core/src/tui/agent_pane.rs`

## Communities (88 total, 12 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.05
Nodes (65): Action, AgentInput, AgentPane, bench_config_load_parse(), bench_sqlite_session_save_load(), bench_tui_frame_render(), Command, CommandPalette (+57 more)

### Community 1 - "Community 1"
Cohesion: 0.06
Nodes (63): AgentBlock, BridgeConfig, LoggingConfig, AgentConfig, Default, HashMap, Option, Path (+55 more)

### Community 2 - "Community 2"
Cohesion: 0.08
Nodes (36): make_native(), make_py(), make_ts(), NativePlugin, Plugin, PluginBackend, PluginInfo, PluginManifest (+28 more)

### Community 3 - "Community 3"
Cohesion: 0.06
Nodes (44): auto_commit_disabled(), auto_commit_disabled_with_real_repo(), AutoCommitResult, branch_isolation_disabled_uses_main_worktree(), clear_snapshots(), create_temp_git_repo(), current_branch_with_repo(), FileSnapshot (+36 more)

### Community 4 - "Community 4"
Cohesion: 0.05
Nodes (42): Frame, Option, Pane, Path, Rect, Result, String, Style (+34 more)

### Community 5 - "Community 5"
Cohesion: 0.09
Nodes (40): RpcErrorBody, AtomicU64, Default, HashMap, Option, Result, Self, String (+32 more)

### Community 6 - "Community 6"
Cohesion: 0.07
Nodes (24): BridgeClient, CompactContextParams, CompactContextResponse, default_system_prompt(), default_system_prompt_is_not_empty(), PromptMessage, PromptParams, PromptResponse (+16 more)

### Community 7 - "Community 7"
Cohesion: 0.09
Nodes (42): Default, Frame, Option, Pane, Rect, Self, String, Vec (+34 more)

### Community 8 - "Community 8"
Cohesion: 0.09
Nodes (41): DiagramEdge, DiagramNode, EdgeStyle, Line, MermaidType, NodeShape, Frame, Option (+33 more)

### Community 9 - "Community 9"
Cohesion: 0.06
Nodes (29): Frame, Into, Option, Pane, Path, PathBuf, Rect, Self (+21 more)

### Community 10 - "Community 10"
Cohesion: 0.11
Nodes (20): approve_and_execute(), approve_single_step(), diff_generation_for_file_edits(), file_edit_detection(), format_for_display_shows_steps(), Plan, plan_lifecycle(), PlanManager (+12 more)

### Community 11 - "Community 11"
Cohesion: 0.09
Nodes (39): Frame, Into, Option, Pane, Rect, String, Vec, active_pane_shows_active_border() (+31 more)

### Community 12 - "Community 12"
Cohesion: 0.08
Nodes (34): active_count_zero_initially(), cancel_all_empty(), cancel_all_on_empty_returns_zero(), cancel_nonexistent_returns_false(), cancel_twice_idempotent(), manager_clone_works(), manager_creates_with_max_capacity(), manager_enforces_max_minimum() (+26 more)

### Community 13 - "Community 13"
Cohesion: 0.09
Nodes (33): AtomicBool, Arc, BridgeClient, Default, Drop, GitManager, JoinHandle, Option (+25 more)

### Community 14 - "Community 14"
Cohesion: 0.10
Nodes (27): Frame, Option, PathBuf, Rect, Self, String, Vec, all_base_commands_have_non_empty_names() (+19 more)

### Community 15 - "Community 15"
Cohesion: 0.12
Nodes (23): agent_loop_detects_compaction_need(), agent_loop_handles_interrupt(), agent_loop_tracks_turns(), AgentLoop, Conversation, estimated_tokens_grows_with_messages(), max_turns_detected(), Message (+15 more)

### Community 16 - "Community 16"
Cohesion: 0.09
Nodes (24): JsonRpcError, CallSkillArgs, CallSkillResult, JsonRpcError, JsonRpcRequest, JsonRpcResponse, RegisterToolArgs, SendPromptArgs (+16 more)

### Community 17 - "Community 17"
Cohesion: 0.13
Nodes (21): build_summarization_prompt_formats_messages(), compact_fallback_summary(), compact_replaces_old_messages(), CompactedSegment, CompactionManager, estimate_total_tokens(), find_compact_segment_identifies_range(), make_messages() (+13 more)

### Community 18 - "Community 18"
Cohesion: 0.11
Nodes (28): Agent, agent_channels(), agent_channels_work(), agent_config_defaults(), AgentConfig, AgentInput, AgentOutput, AgentStatus (+20 more)

### Community 19 - "Community 19"
Cohesion: 0.14
Nodes (17): api_key_detection(), builtin_providers_configured(), custom_provider_registration(), deepseek_config(), glm_config(), ollama_config(), provider_resolution_and_switching(), ProviderConfig (+9 more)

### Community 20 - "Community 20"
Cohesion: 0.11
Nodes (33): GitStatus, Frame, Option, Pane, Rect, String, all_panes_titles_appear_in_status(), bridge_disconnected_shows_text() (+25 more)

### Community 21 - "Community 21"
Cohesion: 0.16
Nodes (18): add_and_retrieve_messages(), create_and_list_sessions(), create_test_store(), import_json_session(), message_count_works(), no_active_session_errors(), save_and_retrieve_plan(), Session (+10 more)

### Community 22 - "Community 22"
Cohesion: 0.13
Nodes (17): Option, Self, String, Style, Vec, compute_with_explicit_language(), detect_syntax_context(), detects_added_and_removed_lines() (+9 more)

### Community 23 - "Community 23"
Cohesion: 0.08
Nodes (37): Agent, agent_config_custom(), agent_config_defaults(), agent_plan_generates_from_messages(), agent_summarize_empty(), agent_with_many_turns(), AgentConfig, AgentOutput (+29 more)

### Community 24 - "Community 24"
Cohesion: 0.08
Nodes (23): For /graphify add and --watch, For /graphify query, For the commit hook and native CLAUDE.md integration, For --update and --cluster-only, /graphify, Honesty Rules, Interpreter guard for subcommands, Part A - Structural extraction for code files (+15 more)

### Community 25 - "Community 25"
Cohesion: 0.14
Nodes (20): Bridge, bridge_starts_and_reads_mock_response(), bridge_times_out_when_child_is_silent(), JsonRpcRequest, JsonRpcResponse, serialize_request(), serializes_json_rpc_request(), AtomicU64 (+12 more)

### Community 26 - "Community 26"
Cohesion: 0.12
Nodes (21): Frame, Rect, String, centered_rect(), centered_rect_computes_correct_area(), centered_rect_full_area(), centered_rect_minimal(), centered_rect_with_different_percents() (+13 more)

### Community 27 - "Community 27"
Cohesion: 0.08
Nodes (23): `agent.max_turns must be > 0` or `must be <= 500`, `Bridge test failed: bridge command is empty`, `bridge.ts_bridge_path '<path>' does not exist`, `cargo run -p rust-core -- --help` does not show usage, CLI Flags, Configuration, Environment Variables, Headless Mode (+15 more)

### Community 28 - "Community 28"
Cohesion: 0.18
Nodes (10): AgentStatus, pool_spawns_and_awaits_multiple_agents(), SubagentPool, SubagentSlot, AgentConfig, AgentOutput, JoinHandle, Self (+2 more)

### Community 29 - "Community 29"
Cohesion: 0.24
Nodes (13): Default, Self, all_toggles_independent(), default_values(), multiple_toggles_in_sequence(), toggle_agent_pane_switches(), toggle_dark_mode_switches(), toggle_file_tree_switches() (+5 more)

### Community 30 - "Community 30"
Cohesion: 0.16
Nodes (8): KeyCode, MouseEvent, KeyEvent, Option, Pane, Action, key(), KeyBindings

### Community 31 - "Community 31"
Cohesion: 0.25
Nodes (9): Message, Result, Self, SqlitePool, String, Vec, save_load_and_list_roundtrip(), SessionInfo (+1 more)

### Community 32 - "Community 32"
Cohesion: 0.26
Nodes (9): Message, message_with_tool_calls(), Role, Into, Option, Self, String, ToolCall (+1 more)

### Community 33 - "Community 33"
Cohesion: 0.36
Nodes (13): build_binary(), main(), now(), process_snapshot(), Path, Popen, read_json_line(), remaining() (+5 more)

### Community 34 - "Community 34"
Cohesion: 0.15
Nodes (12): Crate Responsibilities, Current Gaps and Sharp Edges, Data Flow, Design Decisions, Extension Points, Pi Hybrid Architecture, `py-extensions`, Runtime Entry Points (+4 more)

### Community 35 - "Community 35"
Cohesion: 0.15
Nodes (12): 1. Study the reference, 2. Enhance the TUI (rust-core/src/tui/), 3. Add keybindings (rust-core/src/keybindings.rs), 4. Build the JSON-RPC bridge (rust-core/src/bridge/), 5. Add required Cargo.toml deps, 6. Wire it together in main.rs, 7. Create a bridge test, CONSTRAINTS (+4 more)

### Community 36 - "Community 36"
Cohesion: 0.17
Nodes (11): Module-level Docs Expectations, Operator-facing stable surfaces, Pi Hybrid Public API Docs, Public Item Documentation Expectations, Rustdoc Organization, Stable Surfaces, `ts-bridge` stable library surface, Unstable Or Scaffolded Surfaces (+3 more)

### Community 37 - "Community 37"
Cohesion: 0.17
Nodes (11): 1. Build the Agent Runtime (rust-core/src/agent/), 2. Parallel Subagents (rust-core/src/agent/subagent.rs), 3. Plan → Review → Approve → Execute Flow, 4. Session Persistence (rust-core/src/session/), 5. Wire into the TUI, 6. Add Cargo.toml deps, CONSTRAINTS, PHASE 2 MISSION: Agent Loop + Parallel Subagents (+3 more)

### Community 38 - "Community 38"
Cohesion: 0.36
Nodes (11): build_binary(), main(), now(), process_snapshot(), Path, Popen, read_until(), remaining() (+3 more)

### Community 39 - "Community 39"
Cohesion: 0.20
Nodes (9): 1) Clone the repository, 2) Confirm the workspace layout, 3) Build the workspace, 4) Run the Rust tests that are expected to pass in CI, 5) Create and validate a local config, 6) Smoke-test the TUI entry point, 7) Confirm the clone is ready for work, Pi Hybrid Clone-to-Run Verification (+1 more)

### Community 40 - "Community 40"
Cohesion: 0.20
Nodes (9): Add A Plugin, Add A Provider, Add A Tool, Add A TUI Pane, Clone-To-Run Workflow, Code Conventions, Pi Hybrid Contributor Guide, PR Checklist (+1 more)

### Community 41 - "Community 41"
Cohesion: 0.20
Nodes (9): 1. WAIT for reference clones to complete, 2. Create rust-core source structure, 3. Implement main.rs, 4. Verify everything builds, 5. Create docs/README.md, CONSTRAINTS, PHASE 0 MISSION: Pi Rust-Core Hybrid Workspace Setup, REFERENCE (+1 more)

### Community 42 - "Community 42"
Cohesion: 0.20
Nodes (9): 1. Command Palette (Ctrl+P / Cmd+P), 2. Visual Hierarchy + Context Indicators, 3. Toggle System, 4. Polish Details, 5. Help Popup, CONSTRAINTS, PHASE 1B MISSION: Command Palette + UI/UX Polish, VERIFICATION (+1 more)

### Community 43 - "Community 43"
Cohesion: 0.22
Nodes (8): Benchmarks, Executive Summary, Hardware Matrix, Known Limitations, M1 Mini, M5 Air, Methodology, Raw Data

### Community 44 - "Community 44"
Cohesion: 0.22
Nodes (8): Build Verification, Known Behaviors, Results, sab-air (M5 MacBook Air, 24GB/1TB), sab-mini (M1 Mac Mini, 16GB), Terminal Compatibility Matrix, Test Methodology, Tested Terminals

### Community 45 - "Community 45"
Cohesion: 0.25
Nodes (7): Cases we care about, Evidence in the current codebase, Graceful Degradation Notes, No git repository, No network, Practical rule, Python unavailable

### Community 46 - "Community 46"
Cohesion: 0.25
Nodes (7): B10: Security Audit, B7-B10 Hardening Roadmap, B7: Documentation & Architecture, B8: Cross-Platform Hardening, B9: Performance Benchmarks, Current Status, Success Criteria ("Boring")

### Community 47 - "Community 47"
Cohesion: 0.25
Nodes (7): Crate Breakdown, Dependencies (notable), Memory Profile, Next Steps, Phase 5 Report — Pi Rust-Core Hybrid Optimization, Release Build, Success Criteria

### Community 48 - "Community 48"
Cohesion: 0.33
Nodes (5): Current Cargo features, Feature Flags, Practical guidance, What that means in practice, Why we are not changing the feature contract yet

### Community 49 - "Community 49"
Cohesion: 0.33
Nodes (5): Current Contained Checks, Harness Rules, Inspect Live Process State, Process Containment, Safe Manual Cleanup

### Community 50 - "Community 50"
Cohesion: 0.40
Nodes (4): B8 M1 Mac Mini Validation, Commands run, Results, Scope note

### Community 51 - "Community 51"
Cohesion: 0.33
Nodes (5): Build, Graphify (codebase map for agents), Performance, Pi Rust-Core Hybrid Workspace, Workspace Layout

### Community 53 - "Community 53"
Cohesion: 0.08
Nodes (56): Namespace, bench_prompt(), classify_needle_content(), expected_is_local(), filter_prompts(), fmt_ms(), fmt_tokens(), load_prompt_set() (+48 more)

### Community 60 - "Community 60"
Cohesion: 0.08
Nodes (23): For /graphify add and --watch, For /graphify query, For the commit hook and native CLAUDE.md integration, For --update and --cluster-only, /graphify, Honesty Rules, Interpreter guard for subcommands, Part A - Structural extraction for code files (+15 more)

### Community 61 - "Community 61"
Cohesion: 0.14
Nodes (13): Communication Protocol, Current Baseline, Execution Choice, Final Gate, Global Constraints, Task 1: Lock The Test Target, Task 2: Harden Existing Smoke Scripts For Restricted Runners, Task 3: Add A Scripted TUI Interaction Harness (+5 more)

### Community 62 - "Community 62"
Cohesion: 0.22
Nodes (8): graphify reference: extra exports and benchmark, Step 6b - Wiki (only if --wiki flag), Step 7 - Neo4j export (only if --neo4j or --neo4j-push flag), Step 7a - FalkorDB export (only if --falkordb or --falkordb-push flag), Step 7b - SVG export (only if --svg flag), Step 7c - GraphML export (only if --graphml flag), Step 7d - MCP server (only if --mcp flag), Step 8 - Token reduction benchmark (only if total_words > 5000)

### Community 63 - "Community 63"
Cohesion: 0.22
Nodes (8): Agent handoff (save tokens), CI / keyless (Workload Identity Federation), Graphify semantic extraction via Vertex AI + ADC, One-time setup (local), Prerequisites, Run semantic extraction (this repo), Troubleshooting, What the script does

### Community 64 - "Community 64"
Cohesion: 0.22
Nodes (8): graphify reference: extra exports and benchmark, Step 6b - Wiki (only if --wiki flag), Step 7 - Neo4j export (only if --neo4j or --neo4j-push flag), Step 7a - FalkorDB export (only if --falkordb or --falkordb-push flag), Step 7b - SVG export (only if --svg flag), Step 7c - GraphML export (only if --graphml flag), Step 7d - MCP server (only if --mcp flag), Step 8 - Token reduction benchmark (only if total_words > 5000)

### Community 65 - "Community 65"
Cohesion: 0.33
Nodes (5): For /graphify explain, For /graphify path, graphify reference: query, path, explain, Step 0 — Constrained query expansion (REQUIRED before traversal), Step 1 — Traversal

### Community 66 - "Community 66"
Cohesion: 0.33
Nodes (5): For /graphify explain, For /graphify path, graphify reference: query, path, explain, Step 0 — Constrained query expansion (REQUIRED before traversal), Step 1 — Traversal

### Community 67 - "Community 67"
Cohesion: 0.50
Nodes (3): For /graphify add, For --watch, graphify reference: add a URL and watch a folder

### Community 68 - "Community 68"
Cohesion: 0.50
Nodes (3): For git commit hook, For native CLAUDE.md integration, graphify reference: commit hook and native CLAUDE.md integration

### Community 69 - "Community 69"
Cohesion: 0.50
Nodes (3): For --cluster-only, For --update (incremental re-extraction), graphify reference: incremental update and cluster-only

### Community 70 - "Community 70"
Cohesion: 0.50
Nodes (3): For /graphify add, For --watch, graphify reference: add a URL and watch a folder

### Community 71 - "Community 71"
Cohesion: 0.50
Nodes (3): For git commit hook, For native CLAUDE.md integration, graphify reference: commit hook and native CLAUDE.md integration

### Community 72 - "Community 72"
Cohesion: 0.50
Nodes (3): For --cluster-only, For --update (incremental re-extraction), graphify reference: incremental update and cluster-only

### Community 73 - "Community 73"
Cohesion: 0.50
Nodes (3): OPENAI_API_KEY, OPENAI_BASE_URL, graphify-vertex-extract.sh script

### Community 78 - "Community 78"
Cohesion: 0.50
Nodes (3): Build & verify (rust-core), graphify, Needle benchmark

### Community 81 - "Community 81"
Cohesion: 0.25
Nodes (9): CompletedProcess, assert_prompt_response_shape(), make_send_prompt_request(), NeedleBridgeAdapterTests, parse_adapter_responses(), Path, run_adapter(), run_adapter_completed() (+1 more)

### Community 82 - "Community 82"
Cohesion: 0.40
Nodes (4): 04:10-06:30 | force-pushed-phase-5, 05:37-06:00 | force-pushed-phase-5, 06:16 | force-pushed-phase-5, 15:00 | force-pushed-phase-5

## Knowledge Gaps
- **393 isolated node(s):** `Default`, `Option`, `PromptMessage`, `ToolCallResponse`, `TokenUsage` (+388 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **12 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Sync` connect `Community 2` to `Community 18`, `Community 12`, `Community 5`?**
  _High betweenness centrality (0.021) - this node is a cross-community bridge._
- **Why does `active_border_style()` connect `Community 9` to `Community 11`, `Community 4`, `Community 7`?**
  _High betweenness centrality (0.016) - this node is a cross-community bridge._
- **Why does `SemanticDiff` connect `Community 7` to `Community 0`?**
  _High betweenness centrality (0.012) - this node is a cross-community bridge._
- **What connects `Default`, `Option`, `PromptMessage` to the rest of the system?**
  _394 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.054336468129571575 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.05744888023369036 - nodes in this community are weakly interconnected._
- **Should `Community 2` be split into smaller, more focused modules?**
  _Cohesion score 0.07747747747747748 - nodes in this community are weakly interconnected._