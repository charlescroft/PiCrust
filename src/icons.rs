use gpui_kit::assets::IconNamed;
use gpui_kit::SharedString;

#[derive(Clone, Copy)]
pub struct ZapIcon;

impl IconNamed for ZapIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::Zap.path()
    }
}

#[derive(Clone, Copy)]
pub struct ShieldIcon;

impl IconNamed for ShieldIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::Shield.path()
    }
}

#[derive(Clone, Copy)]
pub struct KeyRoundIcon;

impl IconNamed for KeyRoundIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::KeyRound.path()
    }
}

#[derive(Clone, Copy)]
pub struct DatabaseIcon;

impl IconNamed for DatabaseIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::Database.path()
    }
}

#[derive(Clone, Copy)]
pub struct PencilIcon;

impl IconNamed for PencilIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::Pencil.path()
    }
}

#[derive(Clone, Copy)]
pub struct TrashIcon;

impl IconNamed for TrashIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::Trash.path()
    }
}

#[derive(Clone, Copy)]
pub struct AtSignIcon;

impl IconNamed for AtSignIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::AtSign.path()
    }
}

#[derive(Clone, Copy)]
pub struct LoaderCircleIcon;

impl IconNamed for LoaderCircleIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::LoaderCircle.path()
    }
}

#[derive(Clone, Copy)]
pub struct GitBranchIcon;

impl IconNamed for GitBranchIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::GitBranch.path()
    }
}

#[derive(Clone, Copy)]
pub struct ArchiveIcon;

impl IconNamed for ArchiveIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::Archive.path()
    }
}

#[derive(Clone, Copy)]
pub struct ClockIcon;

impl IconNamed for ClockIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::Clock.path()
    }
}

#[derive(Clone, Copy)]
pub struct PlayIcon;

impl IconNamed for PlayIcon {
    fn path(self) -> SharedString {
        gpui_kit::assets::IconName::Play.path()
    }
}
