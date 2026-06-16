#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    // Window / Application Actions
    ZoomIn,
    ZoomOut,
    CommandPalette,
    SaveFile,
    Escape,

    // Editor Navigation Actions
    MoveLeft { select: bool, word: bool },
    MoveRight { select: bool, word: bool },
    MoveUp { select: bool },
    MoveDown { select: bool },
    MoveToLineStart { select: bool },
    MoveToLineEnd { select: bool },

    // Editor Mutation Actions
    SelectAll,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    DeleteLeft,
    DeleteRight,
    InsertNewLine,
    InsertTab,
    InsertChar(char),

    // Line Manipulation
    MoveLineUp,
    MoveLineDown,
    DuplicateLine,
    DeleteLine,

    // Search
    Find,
    GlobalSearch,
    Split,

    // Multi-cursor
    AddCursorUp,
    AddCursorDown,
}
