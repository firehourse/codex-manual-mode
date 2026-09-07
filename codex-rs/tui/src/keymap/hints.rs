//! Resolve visible shortcut alternatives from the same keymap used for input.

use super::KeymapContext;
use super::RuntimeKeymap;
use super::bindings_for_action;
use super::keymap_action_id;
use super::parse_keybinding;
use super::user_bindings;
use crate::key_hint::ShortcutHint;

impl RuntimeKeymap {
    /// Include alternate keys and chords without exposing internal dispatch tokens.
    pub(crate) fn shortcut_hints(
        &self,
        context: KeymapContext,
        action: &'static str,
    ) -> Vec<ShortcutHint> {
        let Some(action_id) = keymap_action_id(context.config_name(), action) else {
            return Vec::new();
        };
        let Some(bindings) = bindings_for_action(self, context.config_name(), action) else {
            return Vec::new();
        };
        if let Some(specs) = self.chords.configured_specs(action_id) {
            return specs
                .iter()
                .filter_map(|spec| {
                    if let Some((prefix, completion)) = spec.split_once(' ') {
                        Some(ShortcutHint::Chord {
                            prefix: parse_keybinding(prefix)?,
                            completion: parse_keybinding(completion)?,
                        })
                    } else {
                        parse_keybinding(spec).map(ShortcutHint::Single)
                    }
                })
                .collect();
        }

        user_bindings(bindings)
            .iter()
            .copied()
            .map(ShortcutHint::Single)
            .chain(
                self.chords
                    .bindings
                    .iter()
                    .filter(|binding| binding.action == action_id)
                    .map(|binding| ShortcutHint::Chord {
                        prefix: binding.chord.prefix,
                        completion: binding.chord.completion,
                    }),
            )
            .collect()
    }
}
