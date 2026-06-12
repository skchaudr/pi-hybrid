#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toggles {
    pub show_file_tree: bool,
    pub show_agent_pane: bool,
    pub dark_mode: bool,
}

impl Default for Toggles {
    fn default() -> Self {
        Self {
            show_file_tree: true,
            show_agent_pane: true,
            dark_mode: true,
        }
    }
}

impl Toggles {
    pub fn toggle_file_tree(&mut self) {
        self.show_file_tree = !self.show_file_tree;
    }

    pub fn toggle_agent_pane(&mut self) {
        self.show_agent_pane = !self.show_agent_pane;
    }

    pub fn toggle_dark_mode(&mut self) {
        self.dark_mode = !self.dark_mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let t = Toggles::default();
        assert!(t.show_file_tree);
        assert!(t.show_agent_pane);
        assert!(t.dark_mode);
    }

    #[test]
    fn toggle_file_tree_switches() {
        let mut t = Toggles::default();
        t.toggle_file_tree();
        assert!(!t.show_file_tree);
        t.toggle_file_tree();
        assert!(t.show_file_tree);
    }

    #[test]
    fn toggle_agent_pane_switches() {
        let mut t = Toggles::default();
        t.toggle_agent_pane();
        assert!(!t.show_agent_pane);
        t.toggle_agent_pane();
        assert!(t.show_agent_pane);
    }

    #[test]
    fn toggle_dark_mode_switches() {
        let mut t = Toggles::default();
        t.toggle_dark_mode();
        assert!(!t.dark_mode);
        t.toggle_dark_mode();
        assert!(t.dark_mode);
    }

    #[test]
    fn all_toggles_independent() {
        let mut t = Toggles::default();
        t.toggle_file_tree();
        t.toggle_dark_mode();
        assert!(!t.show_file_tree);
        assert!(t.show_agent_pane); // unchanged
        assert!(!t.dark_mode);
    }

    #[test]
    fn toggles_copy_trait() {
        let t1 = Toggles::default();
        let t2 = t1;
        assert_eq!(t1, t2);
    }

    #[test]
    fn toggles_eq_comparison() {
        let mut t1 = Toggles::default();
        let mut t2 = Toggles::default();
        assert_eq!(t1, t2);
        t1.toggle_dark_mode();
        assert_ne!(t1, t2);
        t2.toggle_dark_mode();
        assert_eq!(t1, t2);
    }

    #[test]
    fn toggles_debug_format() {
        let t = Toggles::default();
        let debug = format!("{:?}", t);
        assert!(debug.contains("Toggles"));
    }

    #[test]
    fn multiple_toggles_in_sequence() {
        let mut t = Toggles::default();
        t.toggle_file_tree();
        t.toggle_file_tree();
        t.toggle_agent_pane();
        t.toggle_agent_pane();
        t.toggle_dark_mode();
        t.toggle_dark_mode();
        // All back to defaults
        assert!(t.show_file_tree);
        assert!(t.show_agent_pane);
        assert!(t.dark_mode);
    }

    #[test]
    fn toggles_clone() {
        let mut t1 = Toggles::default();
        t1.toggle_file_tree();
        let t2 = t1.clone();
        assert_eq!(t1, t2);
        assert!(!t2.show_file_tree);
    }
}
