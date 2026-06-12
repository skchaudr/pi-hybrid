//! Mermaid Diagram Rendering — Parse and render Mermaid markdown blocks.
//!
//! Detects ```mermaid blocks in agent output and renders them.
//! Two backends:
//!   a) ASCII art fallback using box-drawing characters (always works).
//!   b) Kitty graphics protocol for image rendering (when terminal supports it).
//!
//! Supports: flowchart, sequence, class, state diagrams.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

/// A parsed Mermaid diagram.
#[derive(Debug, Clone)]
pub struct MermaidDiagram {
    /// The diagram type (flowchart, sequence, class, state).
    pub diagram_type: MermaidType,
    /// The raw Mermaid source code.
    pub source: String,
    /// Parsed nodes/edges for rendering.
    pub nodes: Vec<DiagramNode>,
    /// Parsed edges/connections.
    pub edges: Vec<DiagramEdge>,
    /// The title, if any.
    pub title: Option<String>,
}

/// Types of Mermaid diagrams.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MermaidType {
    Flowchart,
    Sequence,
    Class,
    State,
    Unknown,
}

impl MermaidType {
    pub fn name(&self) -> &'static str {
        match self {
            MermaidType::Flowchart => "Flowchart",
            MermaidType::Sequence => "Sequence",
            MermaidType::Class => "Class",
            MermaidType::State => "State",
            MermaidType::Unknown => "Diagram",
        }
    }

    fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "flowchart" | "graph" | "flowchart lr" | "flowchart td" | "flowchart tb"
            | "flowchart rl" | "flowchart bt" => MermaidType::Flowchart,
            "sequencediagram" | "sequence" => MermaidType::Sequence,
            "classdiagram" | "class" => MermaidType::Class,
            "statediagram" | "state" | "statediagram-v2" => MermaidType::State,
            _ => MermaidType::Unknown,
        }
    }
}

/// A node in a Mermaid diagram.
#[derive(Debug, Clone)]
pub struct DiagramNode {
    /// Node identifier.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Node shape: box, diamond, circle, etc.
    pub shape: NodeShape,
}

/// Shape of a diagram node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeShape {
    Box,
    RoundedBox,
    Diamond,
    Circle,
    Hexagon,
    Parallelogram,
}

/// An edge connecting two nodes.
#[derive(Debug, Clone)]
pub struct DiagramEdge {
    /// Source node id.
    pub from: String,
    /// Target node id.
    pub to: String,
    /// Optional label on the edge.
    pub label: Option<String>,
    /// Arrow style: -->, ---, -.->, ==>
    pub style: EdgeStyle,
}

/// Arrow/edge style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeStyle {
    Arrow,
    Line,
    Dotted,
    Thick,
}

/// Parse Mermaid markdown blocks from text content.
pub fn extract_mermaid_blocks(text: &str) -> Vec<MermaidDiagram> {
    let mut diagrams = Vec::new();
    let mut in_block = false;
    let mut block_lines: Vec<String> = Vec::new();
    let mut diagram_type = MermaidType::Unknown;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```mermaid") || trimmed.starts_with("```mermaid") {
            in_block = true;
            block_lines.clear();
            // Extract type from the rest of the line after ```mermaid
            let rest = trimmed.strip_prefix("```mermaid").unwrap_or("");
            let rest = rest.strip_prefix("``` mermaid").unwrap_or(rest);
            diagram_type = MermaidType::from_str(rest);
            continue;
        }
        if in_block && trimmed == "```" {
            // End of block
            let source = block_lines.join("\n");
            if !source.trim().is_empty() {
                let diagram = parse_mermaid_source(&source, diagram_type);
                diagrams.push(diagram);
            }
            in_block = false;
            block_lines.clear();
            diagram_type = MermaidType::Unknown;
            continue;
        }
        if in_block {
            // Detect diagram type from first content line if not already set
            if diagram_type == MermaidType::Unknown {
                diagram_type = MermaidType::from_str(trimmed);
                if diagram_type != MermaidType::Unknown {
                    continue; // Don't include the type declaration in content
                }
            }
            block_lines.push(line.to_string());
        }
    }

    // Handle unclosed block
    if in_block && !block_lines.is_empty() {
        let source = block_lines.join("\n");
        let diagram = parse_mermaid_source(&source, diagram_type);
        diagrams.push(diagram);
    }

    diagrams
}

/// Parse Mermaid source text into a structured diagram.
fn parse_mermaid_source(source: &str, diagram_type: MermaidType) -> MermaidDiagram {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut title: Option<String> = None;

    for line in source.lines() {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        // Detect title
        if trimmed.starts_with("title ") {
            title = Some(
                trimmed
                    .strip_prefix("title ")
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            );
            continue;
        }

        // Handle node definitions: A[Label], B{Label}, C((Label)), D>Label]
        let node = parse_node(trimmed);
        if let Some(node) = node {
            nodes.push(node);
            continue;
        }

        // Handle edges: A --> B, A --- B, A -.-> B, A ==> B
        let edge = parse_edge(trimmed);
        if let Some(edge) = edge {
            // Also extract node definitions from edge endpoints
            extract_nodes_from_edge(&edge, &mut nodes);
            edges.push(edge);
            continue;
        }
    }

    MermaidDiagram {
        diagram_type,
        source: source.to_string(),
        nodes,
        edges,
        title,
    }
}

/// Parse a single node definition from a Mermaid line.
fn parse_node(line: &str) -> Option<DiagramNode> {
    let trimmed = line.trim();
    // Skip lines that are clearly edges (contain -->, ---, etc.)
    if trimmed.contains("-->") || trimmed.contains("---") || trimmed.contains("==>") {
        return None;
    }

    // Match patterns like:
    // A[Label] - box
    // A(Label) - rounded box
    // A{Label} - diamond
    // A((Label)) - circle
    // A>Label] - hexagon
    // A[/Label/] - parallelogram

    // Box: A[Label]
    if let Some(rest) = trimmed.split_once('[') {
        let id = rest.0.trim().to_string();
        let label = rest.1.trim_end_matches(']').trim_matches('"').to_string();
        if !id.is_empty() {
            return Some(DiagramNode {
                id,
                label,
                shape: NodeShape::Box,
            });
        }
    }

    // Rounded: A(Label)
    if let Some(rest) = trimmed.split_once('(') {
        let id = rest.0.trim().to_string();
        // Check it's not a double-parenthesis circle
        if rest.1.trim().ends_with("))") {
            // Actually a circle, handled below
        } else {
            let label = rest.1.trim_end_matches(')').trim_matches('"').to_string();
            if !id.is_empty() {
                return Some(DiagramNode {
                    id,
                    label,
                    shape: NodeShape::RoundedBox,
                });
            }
        }
    }

    // Diamond: A{Label}
    if let Some(rest) = trimmed.split_once('{') {
        let id = rest.0.trim().to_string();
        let label = rest.1.trim_end_matches('}').trim_matches('"').to_string();
        if !id.is_empty() {
            return Some(DiagramNode {
                id,
                label,
                shape: NodeShape::Diamond,
            });
        }
    }

    // Circle: A((Label))
    if let Some(rest) = trimmed.split_once("((") {
        let id = rest.0.trim().to_string();
        let label = rest.1.trim_end_matches("))").trim_matches('"').to_string();
        if !id.is_empty() {
            return Some(DiagramNode {
                id,
                label,
                shape: NodeShape::Circle,
            });
        }
    }

    None
}

/// Parse an edge definition from a Mermaid line.
fn parse_edge(line: &str) -> Option<DiagramEdge> {
    let trimmed = line.trim();

    // Detect edge style and split
    let (style, separator) = if trimmed.contains("-->") {
        (EdgeStyle::Arrow, "-->")
    } else if trimmed.contains("==>") {
        (EdgeStyle::Thick, "==>")
    } else if trimmed.contains("-.") && trimmed.contains("->") {
        (EdgeStyle::Dotted, "-.->")
    } else if trimmed.contains("---") {
        (EdgeStyle::Line, "---")
    } else {
        return None;
    };

    let parts: Vec<&str> = trimmed.splitn(2, separator).collect();
    if parts.len() != 2 {
        return None;
    }

    let from = parts[0].trim().to_string();
    let to_part = parts[1].trim();

    // Check for edge labels with |text| syntax
    // Pattern: A -->|label text| B → to_part = "|label text| B"
    let (to, label) = if let Some(inner) = to_part.strip_prefix('|') {
        // Strip leading |, then split on next | to get (label, target)
        // "label text| B" or "label text|B"
        if let Some((label_text, target)) = inner.split_once('|') {
            let label = label_text.trim().to_string();
            let target = target.trim().to_string();
            (target, if label.is_empty() { None } else { Some(label) })
        } else {
            (to_part.to_string(), None)
        }
    } else {
        (to_part.to_string(), None)
    };

    if from.is_empty() || to.is_empty() {
        return None;
    }

    Some(DiagramEdge {
        from,
        to,
        label,
        style,
    })
}

/// Extract node definitions from edge endpoints (e.g., A[Label] --> B{Label}).
fn extract_nodes_from_edge(edge: &DiagramEdge, nodes: &mut Vec<DiagramNode>) {
    for endpoint in [&edge.from, &edge.to] {
        // Skip if this node is already in the list
        let id = extract_node_id(endpoint);
        if nodes.iter().any(|n| n.id == id) {
            continue;
        }
        if let Some(node) = parse_node(endpoint) {
            nodes.push(node);
        } else if !id.is_empty() {
            // At minimum, create a simple node with just the ID
            nodes.push(DiagramNode {
                id,
                label: endpoint.to_string(),
                shape: NodeShape::Box,
            });
        }
    }
}

/// Extract the node ID from an endpoint string like "A[Label]" -> "A".
fn extract_node_id(endpoint: &str) -> String {
    let trimmed = endpoint.trim();
    if let Some(pos) = trimmed.find('[') {
        trimmed[..pos].to_string()
    } else if let Some(pos) = trimmed.find('{') {
        trimmed[..pos].to_string()
    } else if let Some(pos) = trimmed.find('(') {
        trimmed[..pos].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Rendering backend enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderBackend {
    /// ASCII art using box-drawing characters.
    Ascii,
    /// Kitty graphics protocol (requires terminal support).
    Kitty,
    /// Auto-detect: try Kitty, fallback to ASCII.
    Auto,
}

/// Detect if the terminal supports the Kitty graphics protocol.
pub fn supports_kitty_protocol() -> bool {
    // Check for Kitty terminal via TERM or KITTY_WINDOW_ID env vars
    std::env::var("KITTY_WINDOW_ID").is_ok()
        || std::env::var("TERM")
            .map(|t| t.contains("kitty"))
            .unwrap_or(false)
}

/// Render a Mermaid diagram to a list of display lines (ASCII art fallback).
pub fn render_diagram_ascii(diagram: &MermaidDiagram) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();

    // Header
    let title = diagram
        .title
        .as_deref()
        .unwrap_or(diagram.diagram_type.name());
    lines.push(Line::styled(
        format!("┌─ Mermaid: {} ─────────────────────────────────┐", title),
        Style::default().fg(Color::Cyan),
    ));

    if diagram.nodes.is_empty() && diagram.edges.is_empty() {
        lines.push(Line::styled(
            "│ (No structured nodes parsed — showing raw source)  │",
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Render nodes
    let node_map: std::collections::HashMap<&str, &DiagramNode> =
        diagram.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    for node in &diagram.nodes {
        let shape_prefix = match node.shape {
            NodeShape::Box => '┌',
            NodeShape::RoundedBox => '╭',
            NodeShape::Diamond => '◇',
            NodeShape::Circle => '○',
            NodeShape::Hexagon => '⬡',
            NodeShape::Parallelogram => '▱',
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {} [{}] ", shape_prefix, node.id),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(node.label.clone()),
        ]));
    }

    // Render edges
    for edge in &diagram.edges {
        let arrow = match edge.style {
            EdgeStyle::Arrow => "──▶",
            EdgeStyle::Line => "────",
            EdgeStyle::Dotted => "···▶",
            EdgeStyle::Thick => "══▶",
        };
        let label = edge
            .label
            .as_ref()
            .map(|l| format!(" |{}| ", l))
            .unwrap_or_default();
        lines.push(Line::from(vec![Span::styled(
            format!("  {} {} {} {}", edge.from, arrow, label, edge.to),
            Style::default().fg(Color::Green),
        )]));
    }

    // Footer
    lines.push(Line::styled(
        format!(
            "└─ {} nodes, {} edges ─────────────────────────────────┘",
            diagram.nodes.len(),
            diagram.edges.len()
        ),
        Style::default().fg(Color::Cyan),
    ));

    lines
}

/// Render a diagram using Kitty graphics protocol.
/// Returns base64-encoded image data that can be sent via Kitty protocol.
pub fn render_diagram_kitty(_diagram: &MermaidDiagram) -> Option<String> {
    // Kitty graphics protocol requires actual image rendering.
    // This is a stub that returns None to trigger ASCII fallback.
    // In production, this would use a Mermaid-to-image renderer.
    None
}

/// Render a diagram with the best available backend.
pub fn render_diagram(diagram: &MermaidDiagram, backend: RenderBackend) -> Vec<Line<'static>> {
    match backend {
        RenderBackend::Kitty => {
            // For now, always fallback to ASCII
            render_diagram_ascii(diagram)
        }
        RenderBackend::Ascii | RenderBackend::Auto => render_diagram_ascii(diagram),
    }
}

/// Check if text contains any Mermaid diagrams.
pub fn has_mermaid_diagrams(text: &str) -> bool {
    text.contains("```mermaid") || text.contains("``` mermaid")
}

/// Widget to render Mermaid diagrams in a TUI pane.
#[derive(Debug)]
pub struct MermaidWidget {
    pub diagrams: Vec<MermaidDiagram>,
    pub active_index: usize,
}

impl MermaidWidget {
    pub fn new(diagrams: Vec<MermaidDiagram>) -> Self {
        Self {
            diagrams,
            active_index: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.diagrams.is_empty()
    }

    pub fn diagram_count(&self) -> usize {
        self.diagrams.len()
    }

    pub fn next_diagram(&mut self) {
        if !self.diagrams.is_empty() {
            self.active_index = (self.active_index + 1) % self.diagrams.len();
        }
    }

    pub fn prev_diagram(&mut self) {
        if !self.diagrams.is_empty() {
            self.active_index = self
                .active_index
                .checked_sub(1)
                .unwrap_or(self.diagrams.len() - 1);
        }
    }

    pub fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        if self.diagrams.is_empty() {
            return;
        }

        let diagram = &self.diagrams[self.active_index];
        let block = Block::default()
            .title(format!(
                " Mermaid: {} ({}/{}) ",
                diagram.diagram_type.name(),
                self.active_index + 1,
                self.diagrams.len()
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let lines = render_diagram_ascii(diagram);
        frame.render_widget(Paragraph::new(lines).block(block), area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_single_mermaid_block() {
        let text = r#"Some text
```mermaid
flowchart LR
    A[Start] --> B[End]
```
More text"#;

        let diagrams = extract_mermaid_blocks(text);
        assert_eq!(diagrams.len(), 1);
        assert_eq!(diagrams[0].diagram_type, MermaidType::Flowchart);
        assert!(diagrams[0].nodes.len() >= 2);
        assert_eq!(diagrams[0].edges.len(), 1);
    }

    #[test]
    fn extract_multiple_blocks() {
        let text = r#"```mermaid
flowchart LR
    A --> B
```
Some text
```mermaid
sequence
    Alice->>Bob: Hello
```"#;

        let diagrams = extract_mermaid_blocks(text);
        assert_eq!(diagrams.len(), 2);
        assert_eq!(diagrams[0].diagram_type, MermaidType::Flowchart);
        assert_eq!(diagrams[1].diagram_type, MermaidType::Sequence);
    }

    #[test]
    fn no_mermaid_blocks() {
        let diagrams = extract_mermaid_blocks("Just regular text, no diagrams.");
        assert!(diagrams.is_empty());
    }

    #[test]
    fn has_mermaid_detection() {
        assert!(has_mermaid_diagrams("```mermaid\nflowchart"));
        assert!(has_mermaid_diagrams("``` mermaid"));
        assert!(!has_mermaid_diagrams("regular text"));
    }

    #[test]
    fn parse_node_box() {
        let node = parse_node("A[Start Here]");
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.id, "A");
        assert_eq!(node.label, "Start Here");
        assert_eq!(node.shape, NodeShape::Box);
    }

    #[test]
    fn parse_node_diamond() {
        let node = parse_node("B{Decision}");
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.id, "B");
        assert_eq!(node.label, "Decision");
        assert_eq!(node.shape, NodeShape::Diamond);
    }

    #[test]
    fn parse_node_circle() {
        let node = parse_node("C((Process))");
        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.id, "C");
        assert_eq!(node.shape, NodeShape::Circle);
    }

    #[test]
    fn parse_edge_arrow() {
        let edge = parse_edge("A --> B");
        assert!(edge.is_some());
        let edge = edge.unwrap();
        assert_eq!(edge.from, "A");
        assert_eq!(edge.to, "B");
        assert_eq!(edge.style, EdgeStyle::Arrow);
    }

    #[test]
    fn parse_edge_with_label() {
        let edge = parse_edge("A -->|Yes| B");
        assert!(edge.is_some());
        let edge = edge.unwrap();
        assert_eq!(edge.from, "A");
        assert_eq!(edge.to, "B");
        assert_eq!(edge.label, Some("Yes".to_string()));
    }

    #[test]
    fn parse_edge_thick() {
        let edge = parse_edge("A ==> B");
        assert!(edge.is_some());
        let edge = edge.unwrap();
        assert_eq!(edge.style, EdgeStyle::Thick);
    }

    #[test]
    fn render_ascii_produces_output() {
        let diagram = MermaidDiagram {
            diagram_type: MermaidType::Flowchart,
            source: "flowchart LR\nA[Start] --> B[End]".to_string(),
            nodes: vec![
                DiagramNode {
                    id: "A".to_string(),
                    label: "Start".to_string(),
                    shape: NodeShape::Box,
                },
                DiagramNode {
                    id: "B".to_string(),
                    label: "End".to_string(),
                    shape: NodeShape::Box,
                },
            ],
            edges: vec![DiagramEdge {
                from: "A".to_string(),
                to: "B".to_string(),
                label: None,
                style: EdgeStyle::Arrow,
            }],
            title: None,
        };

        let lines = render_diagram_ascii(&diagram);
        assert!(lines.len() >= 3);
        assert!(lines[0].to_string().contains("Flowchart"));
    }

    #[test]
    fn mermaid_widget_navigation() {
        let diagrams = vec![
            MermaidDiagram {
                diagram_type: MermaidType::Flowchart,
                source: "flowchart LR".to_string(),
                nodes: vec![],
                edges: vec![],
                title: None,
            },
            MermaidDiagram {
                diagram_type: MermaidType::Sequence,
                source: "sequence".to_string(),
                nodes: vec![],
                edges: vec![],
                title: None,
            },
        ];

        let mut widget = MermaidWidget::new(diagrams);
        assert_eq!(widget.diagram_count(), 2);
        assert_eq!(widget.active_index, 0);

        widget.next_diagram();
        assert_eq!(widget.active_index, 1);

        widget.next_diagram();
        assert_eq!(widget.active_index, 0); // wraparound

        widget.prev_diagram();
        assert_eq!(widget.active_index, 1);
    }
}
