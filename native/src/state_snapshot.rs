//! Send changed rows rather than resending every cached icon on each activation.
use crate::model::State;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

#[derive(Default)]
pub struct Publisher {
    previous: Option<Value>,
}

impl Publisher {
    pub fn message(&mut self, state: &State) -> Value {
        let next = serde_json::to_value(state).expect("browser state is JSON");
        let message = match &self.previous {
            None => json!({"type": "state", "state": &next}),
            Some(previous) => {
                let mut fields = Map::new();
                let mut lists = Map::new();
                let removed: Vec<_> = previous
                    .as_object()
                    .unwrap()
                    .keys()
                    .filter(|key| next.get(*key).is_none())
                    .collect();
                for (key, value) in next.as_object().unwrap() {
                    if previous.get(key) == Some(value) {
                        continue;
                    }
                    if matches!(key.as_str(), "tabs" | "spaces" | "pins" | "favorites") {
                        if let (Some(before), Some(after)) = (
                            previous.get(key).and_then(Value::as_array),
                            value.as_array(),
                        ) {
                            let by_id: HashMap<_, _> = before
                                .iter()
                                .map(|row| (row["id"].as_str().unwrap(), row))
                                .collect();
                            let upsert: Vec<_> = after
                                .iter()
                                .filter(|row| {
                                    by_id.get(row["id"].as_str().unwrap()).copied() != Some(*row)
                                })
                                .collect();
                            let old_order: Vec<_> = before.iter().map(|row| &row["id"]).collect();
                            let order: Vec<_> = after.iter().map(|row| &row["id"]).collect();
                            // An order includes all surviving ids, so it also
                            // describes removals without a second deletion list.
                            let mut patch = json!({"upsert": upsert});
                            if old_order != order {
                                patch["order"] = json!(order);
                            }
                            lists.insert(key.clone(), patch);
                            continue;
                        }
                    }
                    fields.insert(key.clone(), value.clone());
                }
                json!({"type": "statePatch", "fields": fields, "removed": removed, "lists": lists})
            }
        };
        self.previous = Some(next);
        message
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_sends_one_changed_row_and_reordering_keeps_the_full_order() {
        let mut state = State::default();
        for tab in &mut state.tabs {
            tab.favicon = Some("x".repeat(32_000));
        }
        let mut publisher = Publisher::default();
        assert_eq!(publisher.message(&state)["type"], "state");
        state.active_tab = state.tabs[1].id.clone();
        state.tabs[1].last_activated += 1;
        let patch = publisher.message(&state);
        assert_eq!(patch["fields"]["activeTab"], state.active_tab);
        assert_eq!(
            patch["lists"]["tabs"]["upsert"].as_array().unwrap().len(),
            1
        );
        assert!(patch["lists"]["tabs"].get("order").is_none());
        assert!(patch["lists"].get("spaces").is_none());
        assert!(patch.to_string().len() < serde_json::to_string(&state).unwrap().len() / 2);
        state.tabs.swap(0, 1);
        state.tabs.pop();
        let patch = publisher.message(&state);
        assert_eq!(patch["lists"]["tabs"]["upsert"], json!([]));
        assert_eq!(
            patch["lists"]["tabs"]["order"],
            json!(state.tabs.iter().map(|tab| &tab.id).collect::<Vec<_>>())
        );
    }

    #[test]
    fn removed_optional_fields_and_empty_lists_are_explicit() {
        let mut state = State {
            sidebar_width: Some(300.),
            ..State::default()
        };
        let mut publisher = Publisher::default();
        publisher.message(&state);
        state.sidebar_width = None;
        state.tabs.clear();
        let patch = publisher.message(&state);
        assert!(patch["removed"]
            .as_array()
            .unwrap()
            .contains(&json!("sidebarWidth")));
        assert_eq!(patch["lists"]["tabs"]["order"], json!([]));
    }
}
