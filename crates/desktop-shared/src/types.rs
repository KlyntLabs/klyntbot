use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Priority {
    P1,
    P2,
    P3,
    P4,
    P5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Status {
    Todo,
    Doing,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Area {
    All,
    Work,
    Personal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViewMode {
    Table,
    Board,
    Tree,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SidebarItem {
    Chat,
    Tasks,
    Okr,
    Calendar,
    Settings,
}
