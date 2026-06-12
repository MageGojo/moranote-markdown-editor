use gpui::*;

actions!(
    markdown_editor,
    [
        OpenFile,
        OpenFolder,
        SaveFile,
        SaveFileAs,
        NewFile,
        ToggleSidebar,
        ToggleSettings,
        ToggleExport,
        ToggleFocusMode,
        ToggleTypewriterMode,
        ToggleQuickOpen,
        ToggleGlobalSearch,
        ModeSource,
        ModePreview,
        ModeSplit,
        ClosePanel,
    ]
);
