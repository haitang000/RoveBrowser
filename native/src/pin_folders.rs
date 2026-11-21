//! Workspace layout for saved tabs. Live controllers still belong to the tabs.
use crate::model::{new_id, Command, Favorite, State};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PinLayout {
    pub root: Vec<String>,
    pub folders: Vec<PinFolder>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PinFolder {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub collapsed: bool,
    pub pins: Vec<String>,
}

impl PinLayout {
    pub fn folder_for(&self, pin: &str) -> Option<String> {
        self.folders
            .iter()
            .find(|folder| folder.pins.iter().any(|id| id == pin))
            .map(|folder| folder.id.clone())
    }

    fn folder(&self, id: &str) -> Option<&PinFolder> {
        self.folders.iter().find(|folder| folder.id == id)
    }

    fn folder_mut(&mut self, id: &str) -> Result<&mut PinFolder, String> {
        self.folders
            .iter_mut()
            .find(|folder| folder.id == id)
            .ok_or_else(|| "文件夹不存在".into())
    }

    fn entries(&self, folder: Option<&str>) -> Result<&[String], String> {
        match folder {
            Some(id) => self
                .folder(id)
                .map(|folder| folder.pins.as_slice())
                .ok_or_else(|| "文件夹不存在".into()),
            None => Ok(&self.root),
        }
    }

    fn repair(&mut self, pins: &[&Favorite]) {
        let by_id: HashMap<_, _> = pins.iter().map(|pin| (pin.id.as_str(), pin)).collect();
        let mut folder_ids = HashSet::new();
        self.folders.retain(|folder| {
            !folder.id.is_empty()
                && !by_id.contains_key(folder.id.as_str())
                && folder_ids.insert(folder.id.clone())
        });
        let mut seen = HashSet::new();
        let mut add_pin = |destination: &mut Vec<String>, id: String| {
            let Some(pin) = by_id.get(id.as_str()) else {
                return;
            };
            if seen.insert(id.clone()) {
                destination.push(id);
                if let Some(other) = &pin.split_with {
                    if by_id.contains_key(other.as_str()) && seen.insert(other.clone()) {
                        destination.push(other.clone());
                    }
                }
            }
        };
        for folder in &mut self.folders {
            for id in std::mem::take(&mut folder.pins) {
                add_pin(&mut folder.pins, id);
            }
        }
        let mut placed_folders = HashSet::new();
        for id in std::mem::take(&mut self.root) {
            if folder_ids.contains(&id) {
                if placed_folders.insert(id.clone()) {
                    self.root.push(id);
                }
            } else {
                add_pin(&mut self.root, id);
            }
        }
        for folder in &self.folders {
            if !placed_folders.contains(&folder.id) {
                self.root.push(folder.id.clone());
            }
        }
        for pin in pins {
            add_pin(&mut self.root, pin.id.clone());
        }
    }

    fn move_entries(
        &mut self,
        moving: Vec<String>,
        folder: Option<&str>,
        anchor: &[String],
        after: bool,
    ) {
        if anchor.iter().any(|id| moving.contains(id)) {
            return;
        }
        self.root.retain(|id| !moving.contains(id));
        for folder in &mut self.folders {
            folder.pins.retain(|id| !moving.contains(id));
        }
        let entries = match folder {
            Some(id) => &mut self.folder_mut(id).expect("validated folder").pins,
            None => &mut self.root,
        };
        let matches = |id: &String| anchor.contains(id);
        let at = if after {
            entries.iter().rposition(matches).map(|index| index + 1)
        } else {
            entries.iter().position(matches)
        }
        .unwrap_or(entries.len());
        entries.splice(at..at, moving);
    }
}

fn folder_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err("请输入 1–80 个字符的文件夹名称".into());
    }
    Ok(name.into())
}

impl State {
    pub fn sync_pin_layouts(&mut self) {
        self.pin_layouts
            .retain(|space, _| self.spaces.iter().any(|item| &item.id == space));
        for (space, layout) in &mut self.pin_layouts {
            let in_space = |pin: &Favorite| pin.space_id.as_ref() == Some(space);
            let pins: Vec<_> = self.pins.iter().filter(|pin| in_space(pin)).collect();
            layout.repair(&pins);
            let order: HashMap<_, _> = layout
                .root
                .iter()
                .flat_map(|id| {
                    layout
                        .folder(id)
                        .map_or(std::slice::from_ref(id), |folder| folder.pins.as_slice())
                })
                .enumerate()
                .map(|(index, id)| (id.as_str(), index))
                .collect();
            if pins
                .iter()
                .enumerate()
                .all(|(index, pin)| order.get(pin.id.as_str()) == Some(&index))
            {
                continue;
            }
            let mut ordered: Vec<_> = pins.into_iter().cloned().collect();
            ordered.sort_by_key(|pin| order.get(pin.id.as_str()).copied().unwrap_or(usize::MAX));
            for (pin, ordered) in self
                .pins
                .iter_mut()
                .filter(|pin| in_space(pin))
                .zip(ordered)
            {
                *pin = ordered;
            }
        }
        self.pin_layouts
            .retain(|_, layout| !layout.folders.is_empty());
    }

    pub fn place_pin(
        &mut self,
        id: &str,
        space: &str,
        folder: Option<&str>,
        before: Option<&str>,
        after: bool,
    ) -> Result<(), String> {
        if !self.spaces.iter().any(|item| item.id == space) {
            return Err("工作区不存在".into());
        }
        let layout = self.pin_layouts.get(space);
        let moving_folder = layout.is_some_and(|layout| layout.folder(id).is_some());
        if moving_folder && folder.is_some() {
            return Err("请选择固定区中的位置".into());
        }
        if let Some(layout) = layout {
            let entries = layout.entries(folder)?;
            if before.is_some_and(|id| !entries.iter().any(|entry| entry == id)) {
                return Err("目标位置已变化，请重试".into());
            }
        } else {
            if folder.is_some() {
                return Err("文件夹不存在".into());
            }
            if before.is_some_and(|id| {
                !self
                    .pins
                    .iter()
                    .any(|pin| pin.id == id && pin.space_id.as_deref() == Some(space))
            }) {
                return Err("目标位置已变化，请重试".into());
            }
        }
        let anchor = before
            .map(|id| {
                let group = self.group(id);
                if group.is_empty() {
                    vec![id.to_owned()]
                } else {
                    group
                }
            })
            .unwrap_or_default();
        if moving_folder {
            self.pin_layouts.get_mut(space).unwrap().move_entries(
                vec![id.into()],
                None,
                &anchor,
                after,
            );
            return Ok(());
        }
        let saved = self.bookmark_id(id).filter(|id| {
            self.bookmark(id)
                .is_some_and(|pin| pin.space_id.as_deref() == Some(space))
        });
        let saved = match saved {
            Some(id) => id,
            None => self.keep_tab(id, false, Some(space))?,
        };
        let moving = self.group(&saved);
        if let Some(layout) = self.pin_layouts.get_mut(space) {
            layout.move_entries(moving, folder, &anchor, after);
        } else if let Some(before) = before {
            crate::bookmarks::move_before(&mut self.pins, &saved, before, after);
        }
        if let Some(active) = self.active() {
            self.active_space = active.space_id.clone();
        }
        Ok(())
    }

    pub fn apply_pin_folder_command(&mut self, command: &Command) -> Result<bool, String> {
        let (space, id) = match command {
            Command::CreatePinFolder { space_id, name } => {
                let name = folder_name(name)?;
                if !self.spaces.iter().any(|space| space.id == *space_id) {
                    return Err("工作区不存在".into());
                }
                let layout =
                    self.pin_layouts
                        .entry(space_id.clone())
                        .or_insert_with(|| PinLayout {
                            root: self
                                .pins
                                .iter()
                                .filter(|pin| pin.space_id.as_ref() == Some(space_id))
                                .map(|pin| pin.id.clone())
                                .collect(),
                            folders: Vec::new(),
                        });
                let id = new_id();
                layout.root.push(id.clone());
                layout.folders.push(PinFolder {
                    id,
                    name,
                    collapsed: false,
                    pins: Vec::new(),
                });
                return Ok(true);
            }
            Command::PlacePin {
                id,
                space_id,
                folder_id,
                before,
                after,
            } => {
                self.place_pin(
                    id,
                    space_id,
                    folder_id.as_deref(),
                    before.as_deref(),
                    *after,
                )?;
                return Ok(true);
            }
            Command::RenamePinFolder { space_id, id, .. }
            | Command::CollapsePinFolder { space_id, id, .. }
            | Command::RemovePinFolder { space_id, id } => (space_id, id),
            _ => return Ok(false),
        };
        let layout = self.pin_layouts.get_mut(space).ok_or("文件夹不存在")?;
        match command {
            Command::RenamePinFolder { name, .. } => {
                layout.folder_mut(id)?.name = folder_name(name)?
            }
            Command::CollapsePinFolder { collapsed, .. } => {
                layout.folder_mut(id)?.collapsed = *collapsed
            }
            Command::RemovePinFolder { .. } => {
                let index = layout
                    .folders
                    .iter()
                    .position(|folder| &folder.id == id)
                    .ok_or("文件夹不存在")?;
                let at = layout
                    .root
                    .iter()
                    .position(|entry| entry == id)
                    .ok_or("文件夹位置已变化")?;
                layout
                    .root
                    .splice(at..at + 1, layout.folders.remove(index).pins);
            }
            _ => unreachable!(),
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;

    fn create(state: &mut State, space: &str) -> String {
        state
            .apply(&Command::CreatePinFolder {
                space_id: space.into(),
                name: "项目".into(),
            })
            .unwrap();
        state.pin_layouts[space].folders.last().unwrap().id.clone()
    }

    #[test]
    fn folder_layout_restores_and_dissolving_preserves_its_saved_split() {
        let path = std::env::temp_dir().join(format!("rove-pin-folders-{}", new_id()));
        let mut state = State::default();
        assert!(serde_json::to_value(&state)
            .unwrap()
            .get("pinLayouts")
            .is_none());
        let first = state.pins[0].id.clone();
        let second = state.pins[1].id.clone();
        state.pins[0].split_with = Some(second.clone());
        state.pins[1].split_with = Some(first.clone());
        let folder = create(&mut state, "personal");
        state
            .apply(&Command::PlacePin {
                id: second.clone(),
                space_id: "personal".into(),
                folder_id: Some(folder.clone()),
                before: None,
                after: false,
            })
            .unwrap();
        state
            .apply(&Command::CollapsePinFolder {
                space_id: "personal".into(),
                id: folder.clone(),
                collapsed: true,
            })
            .unwrap();
        Storage::open(&path).unwrap().save(&state).unwrap();
        let mut restored = Storage::open(&path).unwrap().load().unwrap();
        let layout = &restored.pin_layouts["personal"];
        assert_eq!(layout.root, std::slice::from_ref(&folder));
        assert_eq!(layout.folders[0].pins, [first.clone(), second.clone()]);
        assert!(layout.folders[0].collapsed);
        restored
            .apply(&Command::RemovePinFolder {
                space_id: "personal".into(),
                id: folder,
            })
            .unwrap();
        assert!(restored.pin_layouts.is_empty());
        assert_eq!(
            restored
                .pins
                .iter()
                .map(|pin| pin.id.as_str())
                .collect::<Vec<_>>(),
            [first.as_str(), second.as_str()]
        );
        assert_eq!(
            restored.pins[0].split_with.as_deref(),
            Some(second.as_str())
        );
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn a_foreign_folder_cannot_take_a_pin_and_moving_the_pin_clears_old_membership() {
        let mut state = State::default();
        let pin = state.pins[0].id.clone();
        let folder = create(&mut state, "personal");
        let other = create(&mut state, "work");
        let place = |folder| Command::PlacePin {
            id: pin.clone(),
            space_id: "personal".into(),
            folder_id: Some(folder),
            before: None,
            after: false,
        };
        assert!(state.apply(&place(other)).is_err());
        state.apply(&place(folder.clone())).unwrap();
        assert_eq!(
            state.pin_layouts["personal"].folders[0].pins,
            std::slice::from_ref(&pin)
        );
        state
            .apply(&Command::Move {
                id: pin.clone(),
                space_id: "work".into(),
            })
            .unwrap();
        assert!(state.pin_layouts["personal"].folders[0].pins.is_empty());
        assert!(state.pin_layouts["work"].root.contains(&pin));
        assert_eq!(
            state.bookmark(&pin).unwrap().space_id.as_deref(),
            Some("work")
        );
    }
}
