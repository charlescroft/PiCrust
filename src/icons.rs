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
