//! [Generator] for creating an initial intent.
//!
//! This module implements logic for creating an initial [IntentInput] for
//! fuzzing.
use std::{cmp::max, collections::HashMap, fs::File};

use libafl::{impl_serdeany, prelude::Generator, state::HasNamedMetadata};
use serde::{Deserialize, Serialize};

use crate::intent_input::{IntentInput, MimeType, ReceiverType};

/// A template for an intent to start mutating, loaded from intent_template.json
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IntentTemplate {
    receiver_type: ReceiverType,
    component: String,
    actions: Vec<String>,
    categories: Vec<String>,
    pub known_extras_keys: HashMap<String, String>,
}

impl_serdeany!(IntentTemplate);

impl IntentTemplate {
    /// Get the package name from the component attribute.
    pub fn package_name(&self) -> String {
        return self.component.split("/").collect::<Vec<&str>>()[0].to_string();
    }

    /// Get the class name from the component attribute.
    pub fn class_name(&self) -> String {
        return self.component.split("/").collect::<Vec<&str>>()[1].to_string();
    }

    pub fn number_of_intents(&self) -> usize {
        return self.actions.len() * max(1, self.categories.len());
    }

    pub fn get_intent_input_for_index(&self, index: usize) -> IntentInput {
        let action_index = index % self.actions.len();
        let category_index = index / max(1, self.actions.len());

        IntentInput {
            receiver_type: self.receiver_type.clone(),
            action: self.actions[action_index].clone(),
            category: self.categories.get(category_index).cloned().unwrap_or_default(),
            component_package: self.package_name(),
            component_class: self.class_name(),

            data: None,
            mime_type: MimeType::TextPlain,
            flags: 0,

            extras: Vec::new(),
        }
    }
}

/// Generates some starting intents based on the data from intent_template.json
pub struct IntentGenerator {
    templates: Vec<IntentTemplate>,
    read_count: u32,
}

impl IntentGenerator {
    pub fn new(config: &str) -> Self {
        // Create empty vec to store the templates
        // If str is a file, read the file and parse the JSON
        if let Ok(dir) = std::fs::read_dir(config) {
            // If str is a directory, read all the files in the directory and parse the JSON
            let mut templates: Vec<IntentTemplate> = Vec::new();
            for entry in dir {
                if let Ok(entry) = entry {
                    let file = File::open(entry.path()).expect("Failed to open intent template file");
                    let template: IntentTemplate =
                        serde_json::from_reader(file).expect("Failed to parse intent template file");
                    if template.receiver_type == ReceiverType::Activity {
                        templates.push(template);
                    }
                }
            }
            if templates.is_empty() {
                panic!("No intent templates found in directory");
            }
            return Self { templates, read_count: 0 };
        } else if let Ok(file) = File::open(config) {
            let template: IntentTemplate =
                serde_json::from_reader(file).expect("Failed to parse intent template file");
            return Self { templates: vec![template], read_count: 0 };
        }

        // If str is not a file or directory, panic
        panic!("Failed to open intent template file");
    }

    /// Get the total number of base intents, a combination of all the actions
    /// and categories.
    pub fn number_of_intents(&self) -> usize {
        return self.templates.iter().map(|t| t.number_of_intents()).sum();
    }

    pub fn package_name(&self) -> String {
        // Return the package name of the first template
        return self.templates[0].package_name();
    }

    pub fn enable_synchronization(&self) -> bool {
        self.templates[0].receiver_type == ReceiverType::Activity
    }

    /// Return whether the receiver of this template is supported.
    pub fn is_supported(&self) -> bool {
        return self.templates[0].receiver_type == ReceiverType::Activity
            || self.templates[0].receiver_type == ReceiverType::BroadcastReceiver;
    }
}

impl<S> Generator<IntentInput, S> for IntentGenerator
where
    S: HasNamedMetadata,
{
    fn generate(&mut self, state: &mut S) -> Result<IntentInput, libafl::Error> {
        // Go through all the templates and generate the intent inputs for each template.
        // Keep in mind that every template generates one or more intent inputs.
        let input = self.templates.iter().flat_map(|t| {
            (0..t.number_of_intents()).map(move |i| t.get_intent_input_for_index(i))
        }).nth(self.read_count as usize).unwrap();

        if !state.has_named_metadata::<IntentTemplate>("intent_template") {
            // Save the template to the state so that we can use it later.
            state.add_named_metadata(self.templates[0].clone(), "intent_template");
        }

        self.read_count += 1;

        Ok(input)
    }
}
