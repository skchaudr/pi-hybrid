use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ratatui::{
    Terminal,
    backend::TestBackend,
    layout::{Constraint, Direction, Layout},
};
use rust_core::{
    agent::message::{Message, Role},
    config::PiConfig,
    session::store::SessionStore,
    tui::{
        Pane,
        agent_pane::{AgentPane, AgentRow},
        editor_pane::EditorPane,
        file_tree::FileTree,
        plan_pane::PlanPane,
        status_bar,
    },
};
use tokio::runtime::Runtime;
use uuid::Uuid;

fn bench_tui_frame_render(c: &mut Criterion) {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("test terminal");
    let file_tree = FileTree::load(".");
    let editor = EditorPane::default();
    let mut agents = AgentPane::default();
    agents.set_agents(vec![
        AgentRow {
            id: "agent-1".to_string(),
            goal: "Review benchmark harness".to_string(),
            status: "running".to_string(),
            turns: 3,
        },
        AgentRow {
            id: "agent-2".to_string(),
            goal: "Collect baseline metrics".to_string(),
            status: "completed".to_string(),
            turns: 5,
        },
    ]);
    let mut plan = PlanPane::default();
    plan.set_lines(vec![
        "Add Criterion benchmarks".to_string(),
        "Document baseline workflow".to_string(),
        "Gate binary size and startup".to_string(),
    ]);

    c.bench_function("tui_frame_render_80x24", |b| {
        b.iter(|| {
            terminal
                .draw(|frame| {
                    let outer = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Min(1), Constraint::Length(1)])
                        .split(frame.area());
                    let columns = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([
                            Constraint::Length(22),
                            Constraint::Min(30),
                            Constraint::Length(26),
                        ])
                        .split(outer[0]);
                    let right = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(columns[2]);

                    file_tree.render(frame, columns[0], Pane::Editor);
                    editor.render(frame, columns[1], Pane::Editor);
                    agents.render(frame, right[0], Pane::Editor);
                    plan.render(frame, right[1], Pane::Editor);
                    status_bar::render(
                        frame,
                        outer[1],
                        Pane::Editor,
                        Some("main"),
                        false,
                        true,
                        Some(agents.running_done_counts()),
                        Some("benchmark"),
                        None,
                        Some("deepseek"),
                    );
                })
                .expect("draw frame");
        });
    });
}

fn bench_sqlite_session_save_load(c: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");
    let store = runtime
        .block_on(SessionStore::open(":memory:"))
        .expect("in-memory sqlite store");
    let messages = vec![
        Message::new(Role::User, "Create a benchmark baseline"),
        Message::new(Role::Assistant, "Benchmark baseline created"),
    ];

    c.bench_with_input(
        BenchmarkId::new("sqlite_session_save_load", messages.len()),
        &messages,
        |b, messages| {
            b.iter(|| {
                let session_id = Uuid::new_v4().to_string();
                runtime.block_on(async {
                    store
                        .save_session(&session_id, messages)
                        .await
                        .expect("save session");
                    store.load_session(&session_id).await.expect("load session")
                })
            });
        },
    );
}

fn bench_config_load_parse(c: &mut Criterion) {
    // Default loading validates built-in provider env vars, so the benchmark
    // supplies dummy keys while still taking the no-file/default path.
    unsafe {
        std::env::set_var("PI_DEEPSEEK_API_KEY", "benchmark-dummy-key");
        std::env::set_var("PI_GLM_API_KEY", "benchmark-dummy-key");
    }

    c.bench_function("config_load_default_no_file", |b| {
        b.iter(|| PiConfig::load(None).expect("default config load"))
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(5));
    targets = bench_tui_frame_render, bench_sqlite_session_save_load, bench_config_load_parse
}
criterion_main!(benches);
